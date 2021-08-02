use anyhow::{Context, Result};
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
    settings.config.repos.list().context("failed listing repos")
}
