use anyhow::{Context, Result};
use clap::{App, Arg, ArgMatches};

use crate::settings::Settings;
use crate::Client;

pub fn cmd() -> App<'static> {
    App::new("new")
        .about("create repo")
        .arg(Arg::new("name").required(true).about("repo name"))
}

pub fn run(args: &ArgMatches, _client: &mut Client, settings: &mut Settings) -> Result<()> {
    let name = args.value_of("name").unwrap();
    settings
        .config
        .repos
        .create(name)
        .context(format!("failed creating repo: {:?}", name))
}
