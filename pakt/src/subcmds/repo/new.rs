use anyhow::{Context, Result};
use clap::{App, Arg, ArgMatches, ArgSettings};

use crate::settings::Settings;
use crate::Client;

#[rustfmt::skip]
pub fn cmd() -> App<'static> {
    App::new("new")
        .about("create repo")
        .arg(Arg::new("name")
            .setting(ArgSettings::Required)
            .about("repo name"))
}

pub async fn run(args: &ArgMatches, _client: &mut Client, settings: &mut Settings) -> Result<()> {
    let name = args.value_of("name").unwrap();
    settings
        .config
        .repos
        .create(name)
        .context(format!("failed creating repo: {:?}", name))
}
