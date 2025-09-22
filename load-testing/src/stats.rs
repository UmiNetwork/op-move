use {
    std::{
        collections::HashMap,
        path::PathBuf,
        time::{Duration, Instant},
    },
    tokio::{
        io::AsyncWriteExt,
        sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
        task::JoinHandle,
    },
};

// Average RPC response times over 30s window
const WINDOW_SIZE: Duration = Duration::from_secs(30);

pub struct Datapoint {
    pub rpc_method: &'static str,
    pub timestamp: Instant,
    pub duration: Duration,
}

pub struct StatsCollector {
    channel: UnboundedReceiver<Datapoint>,
    timing_data: HashMap<&'static str, Windows>,
    write_dir: PathBuf,
    writer_handle: Option<JoinHandle<anyhow::Result<()>>>,
}

impl StatsCollector {
    pub fn new(write_dir: PathBuf) -> (Self, UnboundedSender<Datapoint>) {
        let (sender, channel) = mpsc::unbounded_channel();
        (
            Self {
                channel,
                timing_data: Default::default(),
                write_dir,
                writer_handle: None,
            },
            sender,
        )
    }

    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut buffer = Vec::with_capacity(10);
            loop {
                let read_count = self.channel.recv_many(&mut buffer, 10).await;
                if read_count == 0 {
                    break;
                }
                // Sort the data so that we process earlier timestamps first
                buffer.sort_unstable_by_key(|data| data.timestamp);
                for data in buffer.drain(..) {
                    self.insert_data(data).await;
                }
            }
            // After all data has been collected, do one final write.
            let writer_task = StatsWriter::spawn(self.timing_data, self.write_dir);
            if let Err(e) = writer_task
                .await
                .map_err(anyhow::Error::from)
                .and_then(std::convert::identity)
            {
                println!("WARN: Failed in writing stats: {e:?}");
            }
        })
    }

    async fn insert_data(&mut self, data: Datapoint) {
        let windows = self
            .timing_data
            .entry(data.rpc_method)
            .or_insert_with(|| Windows::new(data.timestamp));
        let avg = windows.insert(data);

        // If the window has hit a specific number of data points
        // then spawn a task to write the stats accumulated so far.
        // Note: data points are processed one at a time so we will hit
        // exactly the required number of points eventually
        // (unless the window never has at least that many points).
        if avg.is_some_and(|x| x.n_data == 50) {
            match self.resolve_writer_handle().await {
                Ok(WriterStatus::Done) => {
                    let writer_task =
                        StatsWriter::spawn(self.timing_data.clone(), self.write_dir.clone());
                    self.writer_handle = Some(writer_task);
                }
                Ok(WriterStatus::InProgress) => {
                    // Intentionally do nothing.
                    // Skip writing this update.
                    // We'll try again on the next one.
                }
                Err(e) => {
                    println!("WARN: Failed in writing stats: {e:?}");
                }
            }
        }
    }

    async fn resolve_writer_handle(&mut self) -> anyhow::Result<WriterStatus> {
        let Some(handle) = self.writer_handle.take() else {
            return Ok(WriterStatus::Done);
        };
        if !handle.is_finished() {
            self.writer_handle = Some(handle);
            return Ok(WriterStatus::InProgress);
        }
        handle.await??;
        Ok(WriterStatus::Done)
    }
}

#[derive(Debug, Clone)]
struct AverageDuration {
    min: Duration,
    max: Duration,
    total: Duration,
    n_data: u32,
}

impl AverageDuration {
    pub fn add(&mut self, duration: Duration) {
        self.total += duration;
        self.n_data += 1;

        if duration < self.min {
            self.min = duration;
        } else if self.max < duration {
            self.max = duration
        }
    }

    pub fn average(&self) -> Duration {
        self.total / self.n_data
    }
}

impl Default for AverageDuration {
    fn default() -> Self {
        Self {
            min: Duration::from_secs(u64::MAX),
            max: Duration::from_nanos(0),
            total: Duration::from_nanos(0),
            n_data: 0,
        }
    }
}

#[derive(Clone)]
struct Windows {
    start: Instant,
    buckets: Vec<AverageDuration>,
}

impl Windows {
    pub fn new(start: Instant) -> Self {
        Self {
            start,
            buckets: Vec::new(),
        }
    }

    pub fn insert(&mut self, data: Datapoint) -> Option<&mut AverageDuration> {
        let Some(duration_since_start) = data.timestamp.checked_duration_since(self.start) else {
            println!("WARN: Earlier point found and discarded");
            return None;
        };
        let n = (duration_since_start.as_secs() / WINDOW_SIZE.as_secs()) as usize;
        if let Some(missing) = (n + 1).checked_sub(self.buckets.len()) {
            for _ in 0..missing {
                self.buckets.push(AverageDuration::default());
            }
        }
        let avg = self
            .buckets
            .get_mut(n)
            .expect("Index in bounds by check above");
        avg.add(data.duration);
        Some(avg)
    }
}

enum WriterStatus {
    InProgress,
    Done,
}

struct StatsWriter {
    timing_data: HashMap<&'static str, Windows>,
    write_dir: PathBuf,
}

impl StatsWriter {
    pub fn spawn(
        timing_data: HashMap<&'static str, Windows>,
        write_dir: PathBuf,
    ) -> JoinHandle<anyhow::Result<()>> {
        let this = Self {
            timing_data,
            write_dir,
        };

        tokio::spawn(async move {
            for (method, windows) in this.timing_data {
                let mut file =
                    tokio::fs::File::create(this.write_dir.join(format!("{method}.csv"))).await?;
                for (i, avg) in windows.buckets.into_iter().enumerate() {
                    if avg.n_data == 0 {
                        continue;
                    }
                    let i = i as u32;
                    let t = (WINDOW_SIZE * i).as_secs();
                    let mn = avg.min.as_micros();
                    let d = avg.average().as_micros();
                    let mx = avg.max.as_micros();
                    file.write_all(format!("{t},{mn},{d},{mx}\n").as_bytes())
                        .await?;
                }
            }

            Ok(())
        })
    }
}
