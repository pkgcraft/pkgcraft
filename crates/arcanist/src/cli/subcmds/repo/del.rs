use anyhow::{Context, Result};
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::Client;
use arcanist::proto::ListRequest;

#[rustfmt::skip]
pub fn cmd() -> Command {
    Command::new("del")
        .about("remove repo(s)")
        .arg(Arg::new("repos")
            .required(true)
            .action(ArgAction::Append)
            .value_name("REPO")
            .help("repos to remove"))
}

pub async fn run(args: &ArgMatches, client: &mut Client) -> Result<()> {
    let repos: Vec<_> = args
        .get_many::<String>("repos")
        .unwrap()
        .map(|s| s.to_string())
        .collect();
    let request = tonic::Request::new(ListRequest { data: repos });
    client
        .remove_repos(request)
        .await
        .context("failed removing repo(s)")?;
    Ok(())
}
