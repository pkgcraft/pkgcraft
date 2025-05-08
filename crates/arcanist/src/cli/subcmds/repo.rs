use anyhow::Result;
use clap::{ArgMatches, Command};

use crate::Client;
use crate::settings::Settings;

mod list;

#[rustfmt::skip]
pub fn cmd() -> Command {
    Command::new("repo")
        .about("manage repos")
        .disable_help_subcommand(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(list::cmd())
}

pub async fn run(args: &ArgMatches, client: &mut Client, _settings: &Settings) -> Result<()> {
    let (subcmd, _) = args.subcommand().unwrap();
    match subcmd {
        "list" => list::run(client).await,
        _ => unreachable!("unknown subcommand"),
    }
}
