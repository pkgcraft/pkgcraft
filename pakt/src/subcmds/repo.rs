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

#[rustfmt::skip]
pub fn cmd() -> App<'static> {
    App::new("repo")
        .about("manage repos")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommands(register())
}

pub async fn run(args: &ArgMatches, client: &mut Client, settings: &mut Settings) -> Result<()> {
    let (subcmd, m) = args.subcommand().unwrap();
    match subcmd {
        "add" => add::run(m, client, settings).await,
        "del" => del::run(m, client, settings).await,
        "list" => list::run(client).await,
        "new" => new::run(m, client, settings).await,
        "sync" => sync::run(m, client, settings).await,
        _ => panic!("unknown subcommand"),
    }
}
