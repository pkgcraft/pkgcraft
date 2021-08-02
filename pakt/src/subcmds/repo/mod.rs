use anyhow::Result;
use clap::{App, AppSettings, ArgMatches};

use crate::settings::Settings;

include!(concat!(env!("OUT_DIR"), "/subcmds/repo/generated.rs"));

pub fn cmd() -> App<'static> {
    App::new("repo")
        .about("manage available repos")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
        .subcommands(register())
}

pub fn run(_args: &ArgMatches, settings: &mut Settings) -> Result<()> {
    for (id, config) in settings.config.repos.configs.iter() {
        println!("{}: {}", id, config.location);
    }
    Ok(())
}
