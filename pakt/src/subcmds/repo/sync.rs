use anyhow::{Context, Result};
use clap::{App, Arg, ArgMatches};

use crate::settings::Settings;

pub fn cmd() -> App<'static> {
    App::new("sync").about("sync repos")
        .arg(Arg::new("repos")
            .required(true)
            .takes_value(true)
            .multiple_values(true)
            .value_name("REPO")
            .about("repos to sync"))
}

pub fn run(args: &ArgMatches, settings: &mut Settings) -> Result<()> {
    let repos: Vec<&str> = args.values_of("repos").unwrap().collect();
    settings
        .config
        .repos
        .sync(&repos)
        .context("failed syncing repo(s)")
}
