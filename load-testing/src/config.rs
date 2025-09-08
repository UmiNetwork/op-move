use {
    crate::CARGO_MANIFEST_DIR,
    jsonwebtoken::EncodingKey,
    std::path::Path,
    tempfile::TempDir,
    umi_server_args::{OptionalAuthSocket, OptionalConfig, OptionalDatabase, OptionalGenesis},
};

pub struct LoadTestConfig {
    db_dir: TempDir,
    jwt_secret: [u8; 4],
}

impl LoadTestConfig {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            db_dir: tempfile::tempdir()?,
            jwt_secret: [0xde, 0xad, 0xbe, 0xef],
        })
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
