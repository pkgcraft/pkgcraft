use std::env;
use std::path::Path;

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
    pub fn new<P: AsRef<Path>>(
        config: &PkgcraftConfig,
        path: Option<P>,
        skip_config: bool,
    ) -> Result<Self> {
        let mut s = Config::builder().add_source(Config::try_from(&Settings::default())?);

        let binary = env!("CARGO_BIN_NAME");
        let binary_upper = binary.to_uppercase();

        // load config file from given location or default fallback if not signalled to skip
        if !skip_config {
            match path {
                Some(path) => {
                    let path = path.as_ref();
                    s = s.add_source(File::from(path).required(true));
                }
                None => {
                    let config_path = config.path().config.join(format!("{binary}.toml"));
                    s = s.add_source(File::from(config_path.as_ref()).required(false));
                }
            }
        }

        // merge env variable overrides
        s = s.add_source(Environment::with_prefix(&binary_upper).separator("_"));

        // respect NO_COLOR -- https://no-color.org/
        if env::var_os("NO_COLOR").is_some() {
            s = s
                .set_override("color", false)
                .context("failed setting color")?;
        }

        // serialize to struct
        let s = s.build().context("failed building config")?;
        let settings: Settings = s.try_deserialize().context("failed serializing settings")?;

        Ok(settings)
    }
}
