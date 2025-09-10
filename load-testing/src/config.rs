use {
    crate::CARGO_MANIFEST_DIR,
    jsonwebtoken::EncodingKey,
    std::{
        borrow::Cow,
        path::{Path, PathBuf},
        time::Duration,
    },
    tempfile::TempDir,
    umi_server_args::{OptionalAuthSocket, OptionalConfig, OptionalDatabase, OptionalGenesis},
};

pub struct LoadTestConfig {
    binary: BinaryPath,
    db_dir: TempDir,
    jwt_secret: [u8; 4],
    pub op_move_start_time: Duration,
    pub n_balance_checkers: usize,
    pub load_test_duration: Duration,
}

impl LoadTestConfig {
    pub fn new() -> anyhow::Result<Self> {
        // An environment variable can be used to pick an existing version
        // of `op-move`. Otherwise it will be compiled fresh from this repository.
        let binary = std::env::var("OP_MOVE_BINARY_PATH")
            .map(|path| BinaryPath::Existing(Path::new(&path).into()))
            .unwrap_or(BinaryPath::Compile);

        Ok(Self {
            binary,
            db_dir: tempfile::tempdir()?,
            jwt_secret: [0xde, 0xad, 0xbe, 0xef],
            op_move_start_time: Duration::from_secs(30),
            n_balance_checkers: 30,
            load_test_duration: Duration::from_secs(5 * 60), // 5 minutes
        })
    }

    pub async fn binary_path(&self) -> anyhow::Result<Cow<PathBuf>> {
        match &self.binary {
            BinaryPath::Existing(path) => Ok(Cow::Borrowed(path)),
            BinaryPath::Compile => {
                let path = crate::compile::build_umi_server().await?;
                Ok(Cow::Owned(path))
            }
        }
    }

    pub fn jwt_secret(&self) -> EncodingKey {
        EncodingKey::from_secret(&self.jwt_secret)
    }

    pub fn to_server_config(&self) -> anyhow::Result<OptionalConfig> {
        let genesis_path = Path::new(CARGO_MANIFEST_DIR)
            .join("../execution/src/tests/res/l2_genesis_tests.json")
            .canonicalize()?;
        Ok(OptionalConfig {
            auth: Some(OptionalAuthSocket {
                addr: None,
                jwt_secret: Some(hex::encode(self.jwt_secret)),
            }),
            db: Some(OptionalDatabase {
                dir: Some(self.db_dir.path().canonicalize()?.into_boxed_path()),
                backend: None,
                purge: Some(true),
            }),
            genesis: Some(OptionalGenesis {
                l2_contract_genesis: Some(genesis_path.into_boxed_path()),
                ..Default::default()
            }),
            ..Default::default()
        })
    }
}

pub enum BinaryPath {
    Compile,
    Existing(PathBuf),
}
