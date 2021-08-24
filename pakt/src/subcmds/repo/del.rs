use anyhow::{Context, Result};
use clap::{App, Arg, ArgMatches, ArgSettings};

use crate::settings::Settings;
use crate::Client;

#[rustfmt::skip]
pub fn cmd() -> App<'static> {
    App::new("del")
        .about("unregister repo")
        .arg(Arg::new("repos")
            .setting(ArgSettings::Required)
            .setting(ArgSettings::TakesValue)
            .setting(ArgSettings::MultipleValues)
            .value_name("REPO")
            .about("repos to remove"))
}

pub async fn run(args: &ArgMatches, _client: &mut Client, settings: &mut Settings) -> Result<()> {
    let repos: Vec<&str> = args.values_of("repos").unwrap().collect();
    settings
        .config
        .repos
        .del(&repos, true)
        .context("failed removing repo(s)")
}
