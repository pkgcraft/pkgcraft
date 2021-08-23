use anyhow::{Context, Result};
use clap::{App, Arg, ArgMatches};

use crate::settings::Settings;
use crate::Client;

#[rustfmt::skip]
pub fn cmd() -> App<'static> {
    App::new("sync")
        .about("sync repos")
        .arg(Arg::new("repos")
            .takes_value(true)
            .multiple_values(true)
            .value_name("REPO")
            .about("repos to sync"))
}

pub fn run(args: &ArgMatches, _client: &mut Client, settings: &mut Settings) -> Result<()> {
    let repos = args.values_of("repos").map(|names| names.collect());
    settings
        .config
        .repos
        .sync(repos)
        .context("failed syncing repo(s)")
}
