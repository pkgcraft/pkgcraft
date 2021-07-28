use anyhow::{anyhow, Result};
use clap::{App, AppSettings, Arg, ArgMatches};

use crate::settings::Settings;

pub fn cmd() -> App<'static> {
    App::new("repo")
        .about("manage available repos")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
        .subcommand(
            App::new("add")
                .about("register new repo")
                .arg(Arg::new("name").required(true).about("repo name"))
                .arg(Arg::new("uri").required(true).about("repo location")),
        )
        .subcommand(
            App::new("del")
                .about("remove known repo")
                .arg(Arg::new("name").required(true).about("repo name")),
        )
}

pub fn run(args: &ArgMatches, settings: &mut Settings) -> Result<()> {
    match args.subcommand() {
        Some(("add", m)) => add(m, settings),
        Some(("del", m)) => del(m, settings),
        _ => Ok(()),
    }
}

fn add(args: &ArgMatches, settings: &mut Settings) -> Result<()> {
    let name = args.value_of("name").unwrap();
    let uri = args.value_of("uri").unwrap();
    match settings.config.repos.add(name, uri) {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!(e)),
    }
}

fn del(args: &ArgMatches, settings: &mut Settings) -> Result<()> {
    let name = args.value_of("name").unwrap();
    match settings.config.repos.del(name) {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!(e)),
    }
}
