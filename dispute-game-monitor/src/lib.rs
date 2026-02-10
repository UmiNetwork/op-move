use {
    crate::{
        config::Config,
        contracts::{
            dispute_game_factory::{
                self,
                DisputeGameFactory::{DisputeGameFactoryInstance, gameAtIndexReturn},
            },
            permissioned_dispute_game,
        },
        state::State,
    },
    alloy::{
        primitives::{Address, U256},
        providers::Provider,
    },
    anyhow::Context,
    std::time::{Duration, SystemTime},
    tracing::level_filters::LevelFilter,
    tracing_subscriber::{filter::EnvFilter, fmt::format::FmtSpan},
};

pub mod cli;
pub mod config;
pub mod state;

mod challenger;
mod contracts;

const MAX_GAME_DURATION: Duration = Duration::from_secs(86_407);

pub fn set_global_tracing_subscriber() {
    // Default to debug level logging, except for hyper and alloy because they are too verbose.
    let filter = EnvFilter::default()
        .add_directive(LevelFilter::DEBUG.into())
        .add_directive("hyper=warn".parse().expect("Is valid directive"))
        .add_directive("alloy=info".parse().expect("Is valid directive"));

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_thread_names(true)
        .with_env_filter(filter)
        .with_ansi(false)
        .with_span_events(FmtSpan::FULL)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");
}

pub async fn initialize(config: &Config) -> anyhow::Result<(State, impl Provider + 'static)> {
    let provider = config.get_provider();
    let game_factory =
        dispute_game_factory::DisputeGameFactory::new(config.factory_address, &provider);
    let game_count: u64 = game_factory
        .gameCount()
        .call()
        .await
        .context("Failed to query game count from dispute factory contract")?
        .saturating_to();

    let mut state = State::empty_queue(game_count);
    let Some((mut index, mut game_info)) =
        latest_in_progress_game(&game_factory, &provider, game_count).await?
    else {
        tracing::info!("No games currently in progress, skipping to main loop.");
        return Ok((state, provider));
    };

    tracing::info!(
        "Latest in-progress game: index={index} address={}",
        game_info.proxy_
    );
    let can_resolve_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(u64::MAX);
    while index < game_count {
        let resolve_time = game_info.timestamp_ + MAX_GAME_DURATION.as_secs();
        let resolve_outcome = if resolve_time < can_resolve_secs {
            try_resolve_game(&game_info.proxy_, config).await
        } else {
            ResolveOutcome::ClaimResolveFailed(anyhow::Error::msg("Not ready."))
        };
        match resolve_outcome {
            ResolveOutcome::ClaimResolveFailed(_) => {
                tracing::info!(
                    "Adding in-progress game to queue: index={index} address={}",
                    game_info.proxy_
                );
                state.push(game_info.proxy_, resolve_time);
            }
            ResolveOutcome::Success => (),
            ResolveOutcome::GameResolveFailed(e) => {
                anyhow::bail!("Error resolving permissioned dispute game: {e:?}");
            }
        }
        index += 1;
        if index < game_count {
            game_info = get_game_at_index(&game_factory, index).await?;
        }
    }

    Ok((state, provider))
}

pub async fn monitor_loop<P>(config: Config, mut state: State, provider: P) -> anyhow::Result<()>
where
    P: Provider + 'static,
{
    let game_factory =
        dispute_game_factory::DisputeGameFactory::new(config.factory_address, &provider);
    'outer: loop {
        tokio::time::sleep(config.interval).await;

        let now = SystemTime::now();
        while let Some((address, timestamp)) = state.awaiting_resolution.front()
            && timestamp < &now
        {
            match try_resolve_game(address, &config).await {
                ResolveOutcome::Success => (),
                ResolveOutcome::ClaimResolveFailed(e) => {
                    tracing::warn!("Error resolving claim on permissioned dispute game: {e:?}");
                    // We leave this game in the queue to try again in the future.
                    break;
                }
                ResolveOutcome::GameResolveFailed(e) => {
                    // This is an error because we do not want to leave the game
                    // in this half-resolved state. Manual intervention is required
                    // to fix it before continuing.
                    anyhow::bail!("Error resolving permissioned dispute game: {e:?}");
                }
            }
            state.awaiting_resolution.pop_front();
        }

        let new_game_count: u64 = match game_factory.gameCount().call().await {
            Ok(new_game_count) => new_game_count.saturating_to(),
            Err(e) => {
                tracing::warn!("Failed to read game count from dispute game factory: {e:?}");
                continue;
            }
        };

        if new_game_count <= state.game_count {
            // No new games to add
            continue;
        }

        let mut index = state.game_count;
        tracing::info!("Updating internal state with new game count {new_game_count}");
        state.game_count = new_game_count;
        while index < new_game_count {
            let game_info = match get_game_at_index(&game_factory, index).await {
                Ok(info) => info,
                Err(e) => {
                    tracing::warn!(
                        "Failed to read game at index {index} from dispute game factory: {e:?}"
                    );
                    continue 'outer;
                }
            };
            tracing::info!(
                "Adding in-progress game to queue: index={index} address={}",
                game_info.proxy_
            );
            state.push(
                game_info.proxy_,
                game_info.timestamp_ + MAX_GAME_DURATION.as_secs(),
            );
            index += 1;
        }
    }
}

async fn try_resolve_game(address: &Address, config: &Config) -> ResolveOutcome {
    tracing::info!("Attempting to resolve game at address {address}...");

    if let Err(e) = challenger::resolve_claim(address, &config.rpc_url, &config.signer).await {
        return ResolveOutcome::ClaimResolveFailed(e);
    }

    if let Err(e) = challenger::resolve_game(address, &config.rpc_url, &config.signer).await {
        return ResolveOutcome::GameResolveFailed(e);
    }

    tracing::info!("Game at address {address} fully resolved.");
    ResolveOutcome::Success
}

async fn latest_in_progress_game<P: Provider + Copy>(
    game_factory: &DisputeGameFactoryInstance<P>,
    provider: P,
    game_count: u64,
) -> anyhow::Result<Option<(u64, gameAtIndexReturn)>> {
    // There are no games to check
    if game_count == 0 {
        return Ok(None);
    }

    // If the first game is unresolved then all subsequent games are too.
    let first_game = get_game_at_index(game_factory, 0).await?;
    if let GameStatus::InProgress = game_status(first_game.proxy_, provider).await? {
        return Ok(Some((0, first_game)));
    }

    // If the last game is resolved then there are no in progress games.
    let last_game = get_game_at_index(game_factory, game_count - 1).await?;
    if matches!(
        game_status(last_game.proxy_, provider).await?,
        GameStatus::ChallengerWins | GameStatus::DefenderWins
    ) {
        return Ok(None);
    }

    // Binary search for latest in-progress game.
    let mut lower = 0;
    let mut upper = game_count - 1;
    let mut game_info = last_game;
    while upper - lower > 1 {
        let index = (lower + upper) / 2;
        let game = get_game_at_index(game_factory, index).await?;
        match game_status(game.proxy_, provider).await? {
            GameStatus::InProgress => {
                upper = index;
                game_info = game;
            }
            GameStatus::DefenderWins | GameStatus::ChallengerWins => {
                lower = index;
            }
        }
    }

    Ok(Some((upper, game_info)))
}

async fn get_game_at_index<P: Provider>(
    game_factory: &DisputeGameFactoryInstance<P>,
    index: u64,
) -> anyhow::Result<gameAtIndexReturn> {
    game_factory
        .gameAtIndex(U256::from(index))
        .call()
        .await
        .context("Failed to get game at index 0 from dispute factory contract")
}

async fn game_status<P: Provider>(
    game_address: Address,
    provider: P,
) -> anyhow::Result<GameStatus> {
    let game_contract =
        permissioned_dispute_game::PermissionedDisputeGame::new(game_address, provider);
    let status = game_contract
        .status()
        .call()
        .await
        .context("Failed to query status of permissioned dispute game contract")?;
    status.try_into()
}

enum ResolveOutcome {
    Success,
    ClaimResolveFailed(anyhow::Error),
    GameResolveFailed(anyhow::Error),
}

#[repr(u8)]
enum GameStatus {
    InProgress = 0,
    ChallengerWins = 1,
    DefenderWins = 2,
}

impl TryFrom<u8> for GameStatus {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::InProgress),
            1 => Ok(Self::ChallengerWins),
            2 => Ok(Self::DefenderWins),
            other => Err(anyhow::Error::msg(format!(
                "Unknown permissioned dispute game status {other}"
            ))),
        }
    }
}
