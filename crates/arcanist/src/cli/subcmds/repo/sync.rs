use anyhow::{Context, Result};
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::Client;
use arcanist::proto::ListRequest;

#[rustfmt::skip]
pub fn cmd() -> Command {
    Command::new("sync")
        .about("sync repos")
        .arg(Arg::new("repos")
            .action(ArgAction::Append)
            .value_name("REPO")
            .help("repos to sync"))
}

pub async fn run(args: &ArgMatches, client: &mut Client) -> Result<()> {
    let repos: Vec<_> = args
        .get_many::<String>("repos")
        .map(|names| names.map(|s| s.to_string()).collect())
        .unwrap_or_default();

    let request = tonic::Request::new(ListRequest { data: repos });
    client
        .sync_repos(request)
        .await
        .context("failed syncing repo(s)")?;

    Ok(())
}
