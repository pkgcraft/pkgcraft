use anyhow::{Context, Result};
use config::{Config, Environment};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Settings {
    pub debug: bool,
    pub verbosity: i32,
    pub socket: Option<String>,
}

impl Settings {
    pub fn new() -> Result<Self> {
        let mut s = Config::default();

        // use defaults
        s.merge(Config::try_from(&Settings::default())?)
            .context("failed merging config defaults")?;

        // merge env variable overrides
        let bin = env!("CARGO_BIN_NAME").to_uppercase();
        s.merge(Environment::with_prefix(&bin).separator("_"))
            .context("failed merging env settings")?;

        // serialize to struct
        let settings: Settings = s.try_into().context("failed serializing settings")?;

        Ok(settings)
    }
}
