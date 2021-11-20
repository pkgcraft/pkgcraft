use std::path::Path;

use anyhow::{Context, Result};
use config::{Config, Environment, File};
use pkgcraft::config::Config as PkgcraftConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Settings {
    pub debug: bool,
    pub verbosity: i32,
    pub socket: String,
}

impl Settings {
    pub fn new<P: AsRef<Path>>(
        config: &PkgcraftConfig,
        path: Option<P>,
        skip_config: bool,
    ) -> Result<Self> {
        let mut s = Config::default();

        // use defaults
        s.merge(Config::try_from(&Settings::default())?)
            .context("failed merging config defaults")?;

        let binary = env!("CARGO_BIN_NAME");
        let binary_upper = binary.to_uppercase();

        // load config file from given location or default fallback if not signalled to skip
        if !skip_config {
            match path {
                Some(path) => {
                    let path = path.as_ref();
                    s.merge(File::from(path).required(true))
                        .context(format!("failed merging config settings: {:?}", path))?;
                }
                None => {
                    let config_path = config.path.config.join(format!("{}.toml", &binary));
                    s.merge(File::from(config_path.as_path()).required(false))
                        .context(format!(
                            "failed merging config settings: {:?}",
                            &config_path
                        ))?;
                }
            }
        }

        // merge env variable overrides
        s.merge(Environment::with_prefix(&binary_upper).separator("_"))
            .context("failed merging env settings")?;

        // serialize to struct
        let settings: Settings = s.try_into().context("failed serializing settings")?;

        Ok(settings)
    }
}
