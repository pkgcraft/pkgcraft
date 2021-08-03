use anyhow::Result;
use clap::{App, AppSettings, ArgMatches};

use crate::settings::Settings;

include!(concat!(env!("OUT_DIR"), "/subcmds/repo/generated.rs"));

pub fn cmd() -> App<'static> {
    App::new("repo")
        .about("manage available repos")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommands(register())
}

pub fn run(_args: &ArgMatches, _settings: &mut Settings) -> Result<()> {
    Ok(())
}
