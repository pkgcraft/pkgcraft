use anyhow::Result;
use clap::{App, AppSettings, ArgMatches};

use crate::settings::Settings;
use crate::Client;

mod add;
mod del;
mod list;
mod new;
mod sync;

#[rustfmt::skip]
pub fn register() -> Vec<App<'static>> {
    vec![
        add::cmd(),
        del::cmd(),
        list::cmd(),
        new::cmd(),
        sync::cmd(),
    ]
}

pub fn cmd() -> App<'static> {
    App::new("repo")
        .about("manage repos")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommands(register())
}

pub fn run(args: &ArgMatches, client: &mut Client, settings: &mut Settings) -> Result<()> {
    let (subcmd, m) = args.subcommand().unwrap();
    match subcmd {
        "add" => add::run(m, client, settings),
        "del" => del::run(m, client, settings),
        "list" => list::run(m, client, settings),
        "new" => new::run(m, client, settings),
        "sync" => sync::run(m, client, settings),
        _ => panic!("unknown subcommand"),
    }
}
