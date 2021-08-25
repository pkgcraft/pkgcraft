use std::env;

use anyhow::{Context, Result};
use config::{Config, Environment};
use pkgcraft::config::Config as PkgcraftConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Settings {
    pub color: bool,
    pub debug: bool,
    pub verbosity: i32,
    pub config: PkgcraftConfig,
}

impl Settings {
    pub fn new() -> Result<Self> {
        let mut s = Config::default();

        // use defaults
        s.merge(Config::try_from(&Settings::default())?)
            .context("failed merging config defaults")?;

        // env variables matching PACT_* override
        s.merge(Environment::with_prefix("PACT").separator("_"))
            .context("failed merging env settings")?;

        // respect NO_COLOR -- https://no-color.org/
        if env::var_os("NO_COLOR").is_some() {
            s.set("color", false).context("failed setting color")?;
        }

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
