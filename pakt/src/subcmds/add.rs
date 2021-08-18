use clap::{App, AppSettings};

pub fn cmd() -> App<'static> {
    App::new("add")
        .about("add packages")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommands(register())
}

include!(concat!(env!("OUT_DIR"), "/subcmds/add.rs"));
