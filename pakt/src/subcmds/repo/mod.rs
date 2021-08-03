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

pub fn run(args: &ArgMatches, settings: &mut Settings) -> Result<()> {
    let (subcmd, m) = args.subcommand().unwrap();
    let func = FUNC_MAP.get(subcmd).unwrap();
    func(m, settings)
}
