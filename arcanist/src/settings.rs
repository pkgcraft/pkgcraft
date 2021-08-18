use anyhow::{Context, Result};
use config::{Config, Environment};
use pkgcraft::config::Config as PkgcraftConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub debug: bool,
    pub verbosity: i32,
    pub config: PkgcraftConfig,
    pub port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            debug: false,
            verbosity: 0,
            config: PkgcraftConfig::default(),
            port: 24842,
        }
    }
}

impl Settings {
    pub fn new() -> Result<Self> {
        let mut s = Config::default();

        // use defaults
        s.merge(Config::try_from(&Settings::default())?)
            .context("failed merging config defaults")?;

        // env variables matching ARCANIST_* override
        s.merge(Environment::with_prefix("ARCANIST").separator("_"))
            .context("failed merging env settings")?;

        // serialize to struct
        let settings: Settings = s.try_into().context("failed serializing settings")?;

        Ok(settings)
    }

    // load pkgcraft config
    pub fn load(&mut self) -> Result<()> {
        self.config =
            PkgcraftConfig::new("pkgcraft", "", false).context("failed loading pkgcraft config")?;
        Ok(())
    }
}
