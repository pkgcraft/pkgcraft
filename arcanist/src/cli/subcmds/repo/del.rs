use anyhow::{Context, Result};
use clap::{App, Arg, ArgMatches, ArgSettings};

use crate::arcanist::ListRequest;
use crate::Client;

#[rustfmt::skip]
pub fn cmd() -> App<'static> {
    App::new("del")
        .about("remove repo(s)")
        .arg(Arg::new("repos")
            .setting(ArgSettings::Required)
            .setting(ArgSettings::TakesValue)
            .setting(ArgSettings::MultipleValues)
            .value_name("REPO")
            .about("repos to remove"))
}

pub async fn run(args: &ArgMatches, client: &mut Client) -> Result<()> {
    let repos: Vec<String> = args
        .values_of("repos")
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
