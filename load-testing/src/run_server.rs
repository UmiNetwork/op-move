use {
    crate::CARGO_MANIFEST_DIR,
    std::{fs::File, path::Path},
    tokio::process::{Child, Command},
    umi_server_args::OptionalConfig,
};

const PREFIX: &str = "OP_MOVE";

pub fn start(path: &Path, config: OptionalConfig) -> anyhow::Result<Child> {
    let log_file = File::create(Path::new(CARGO_MANIFEST_DIR).join("op_move.log"))?;
    let child = Command::new(path)
        .envs(config_to_env_vars(config))
        .stdout(log_file)
        .spawn()?;
    Ok(child)
}

fn config_to_env_vars(config: OptionalConfig) -> impl Iterator<Item = (String, String)> {
    let mut vars = Vec::new();

    if let Some(auth) = config.auth {
        if let Some(addr) = auth.addr {
            vars.push((format!("{PREFIX}_AUTH_ADDR"), addr.to_string()));
        }
        if let Some(secret) = auth.jwt_secret {
            vars.push((format!("{PREFIX}_AUTH_JWT_SECRET"), secret));
        }
    }

    if let Some(http) = config.http
        && let Some(addr) = http.addr
    {
        vars.push((format!("{PREFIX}_HTTP_ADDR"), addr.to_string()));
    }

    if let Some(db) = config.db {
        if let Some(backend) = db.backend {
            vars.push((format!("{PREFIX}_DB_BACKEND"), format!("{backend:?}")));
        }
        if let Some(dir) = db.dir {
            vars.push((format!("{PREFIX}_DB_DIR"), path_to_string(&dir)));
        }
        if let Some(purge) = db.purge {
            vars.push((format!("{PREFIX}_DB_PURGE"), format!("{purge:?}")));
        }
    }

    if let Some(genesis) = config.genesis {
        if let Some(chain_id) = genesis.chain_id {
            vars.push((format!("{PREFIX}_GENESIS_CHAIN_ID"), chain_id.to_string()));
        }
        if let Some(initial_state_root) = genesis.initial_state_root {
            vars.push((
                format!("{PREFIX}_GENESIS_INITIAL_STATE_ROOT"),
                format!("{initial_state_root:?}"),
            ));
        }
        if let Some(treasury) = genesis.treasury {
            vars.push((
                format!("{PREFIX}_GENESIS_TREASURY"),
                treasury.to_standard_string(),
            ));
        }
        if let Some(l2_contract_genesis) = genesis.l2_contract_genesis {
            vars.push((
                format!("{PREFIX}_GENESIS_L2_CONTRACT_GENESIS"),
                path_to_string(&l2_contract_genesis),
            ));
        }
        if let Some(token_list) = genesis.token_list {
            vars.push((
                format!("{PREFIX}_GENESIS_TOKEN_LIST"),
                path_to_string(&token_list),
            ));
        }
    }

    if let Some(max_buffered_commands) = config.max_buffered_commands {
        vars.push((
            format!("{PREFIX}_MAX_BUFFERED_COMMANDS"),
            max_buffered_commands.to_string(),
        ));
    }

    vars.into_iter()
}

fn path_to_string(path: &Path) -> String {
    path.to_str().expect("Path should be valid UTF-8").into()
}
