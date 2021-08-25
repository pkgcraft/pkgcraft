use std::env;

use anyhow::{Context, Result};
use config::{Config, Environment, File};
use pkgcraft::config::Config as PkgcraftConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Settings {
    pub color: bool,
    pub debug: bool,
    pub verbosity: i32,
    pub url: String,
}

impl Settings {
    pub fn new(config: &PkgcraftConfig) -> Result<Self> {
        let mut s = Config::default();

        // use defaults
        s.merge(Config::try_from(&Settings::default())?)
            .context("failed merging config defaults")?;

        // load config from file
        let binary = env!("CARGO_BIN_NAME");
        let config_path = config.path.config.join(format!("{}.toml", &binary));
        s.merge(File::from(config_path.as_path()).required(false))
            .context(format!(
                "failed merging config settings: {:?}",
                &config_path
            ))?;

        // merge env variable overrides
        s.merge(Environment::with_prefix(&binary.to_uppercase()).separator("_"))
            .context("failed merging env settings")?;

        // respect NO_COLOR -- https://no-color.org/
        if env::var_os("NO_COLOR").is_some() {
            s.set("color", false).context("failed setting color")?;
        }

        // serialize to struct
        let settings: Settings = s.try_into().context("failed serializing settings")?;

        Ok(settings)
    }
}
