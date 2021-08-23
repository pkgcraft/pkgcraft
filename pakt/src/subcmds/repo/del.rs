use anyhow::{Context, Result};
use clap::{App, Arg, ArgMatches};

use crate::settings::Settings;
use crate::Client;

#[rustfmt::skip]
pub fn cmd() -> App<'static> {
    App::new("del")
        .about("unregister repo")
        .arg(Arg::new("repos")
            .required(true)
            .takes_value(true)
            .multiple_values(true)
            .value_name("REPO")
            .about("repos to remove"))
}

pub fn run(args: &ArgMatches, _client: &mut Client, settings: &mut Settings) -> Result<()> {
    let repos: Vec<&str> = args.values_of("repos").unwrap().collect();
    settings
        .config
        .repos
        .del(&repos, true)
        .context("failed removing repo(s)")
}
