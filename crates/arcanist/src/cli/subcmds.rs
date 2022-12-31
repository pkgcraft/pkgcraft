use anyhow::Result;
use clap::{ArgMatches, Command};

use crate::settings::Settings;
use crate::Client;

mod add;
mod del;
mod repo;
mod search;
mod version;

#[rustfmt::skip]
pub fn register() -> Vec<Command> {
    vec![
        add::cmd(),
        del::cmd(),
        repo::cmd(),
        search::cmd(),
        version::cmd(),
    ]
}

pub async fn run(args: &ArgMatches, client: &mut Client, settings: &Settings) -> Result<()> {
    let (subcmd, m) = args.subcommand().unwrap();
    match subcmd {
        "add" => add::run(m, client).await,
        "del" => del::run(m, client).await,
        "repo" => repo::run(m, client, settings).await,
        "search" => search::run(m, client).await,
        "version" => version::run(client).await,
        _ => panic!("unknown subcommand"),
    }
}
