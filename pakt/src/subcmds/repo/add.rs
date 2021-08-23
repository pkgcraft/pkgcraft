use anyhow::{Context, Result};
use clap::{App, Arg, ArgMatches};

use crate::settings::Settings;
use crate::Client;

pub fn cmd() -> App<'static> {
    App::new("add")
        .about("register repo")
        .arg(Arg::new("name").required(true).about("repo name"))
        .arg(Arg::new("uri").required(true).about("repo location"))
}

pub fn run(args: &ArgMatches, _client: &mut Client, settings: &mut Settings) -> Result<()> {
    let name = args.value_of("name").unwrap();
    let uri = args.value_of("uri").unwrap();
    settings
        .config
        .repos
        .add(name, uri)
        .context(format!("failed adding repo: {:?}", name))
}
