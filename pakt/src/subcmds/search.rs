use anyhow::Result;
use clap::{App, AppSettings, Arg, ArgMatches, ArgSettings};

use crate::arcanist::ListRequest;
use crate::Client;

#[rustfmt::skip]
pub fn cmd() -> App<'static> {
    App::new("search")
        .about("search repos")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
        .arg(Arg::new("pkgs")
            .setting(ArgSettings::TakesValue)
            .setting(ArgSettings::MultipleValues)
            .value_name("TARGET")
            .about("extended atom matching"))
}

pub async fn run(args: &ArgMatches, client: &mut Client) -> Result<()> {
    let pkgs: Vec<String> = args
        .values_of("pkgs")
        .unwrap()
        .map(|s| s.to_string())
        .collect();
    let request = tonic::Request::new(ListRequest { data: pkgs });
    let response = client.search_packages(request).await?;
    let mut stream = response.into_inner();
    while let Some(response) = stream.message().await? {
        println!("{}", response.data);
    }
    Ok(())
}
