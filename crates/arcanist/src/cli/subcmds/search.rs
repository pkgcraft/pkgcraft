use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::Client;
use arcanist::proto::ListRequest;

#[rustfmt::skip]
pub fn cmd() -> Command {
    Command::new("search")
        .about("search repos")
        .disable_help_subcommand(true)
        .arg(Arg::new("pkgs")
            .required(true)
            .action(ArgAction::Append)
            .value_name("TARGET")
            .help("extended atom matching"))
}

pub async fn run(args: &ArgMatches, client: &mut Client) -> Result<()> {
    let pkgs: Vec<_> = args
        .get_many::<String>("pkgs")
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
