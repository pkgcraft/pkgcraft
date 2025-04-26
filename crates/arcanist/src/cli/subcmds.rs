use anyhow::Result;
use clap::{ArgMatches, Command};

use crate::Client;
use crate::settings::Settings;

mod add;
mod del;
mod repo;
mod search;
mod version;

#[rustfmt::skip]
pub fn register() -> impl Iterator<Item = Command> {
    [
        add::cmd(),
        del::cmd(),
        repo::cmd(),
        search::cmd(),
        version::cmd(),
    ].into_iter()
}

pub async fn run(args: &ArgMatches, client: &mut Client, settings: &Settings) -> Result<()> {
    let (subcmd, m) = args.subcommand().unwrap();
    match subcmd {
        "add" => add::run(m, client).await,
        "del" => del::run(m, client).await,
        "repo" => repo::run(m, client, settings).await,
        "search" => search::run(m, client).await,
        "version" => version::run(client).await,
        _ => unreachable!("unknown subcommand"),
    }
}
