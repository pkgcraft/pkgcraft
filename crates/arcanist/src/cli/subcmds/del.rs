use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::Client;
use arcanist::proto::ListRequest;

#[rustfmt::skip]
pub fn cmd() -> Command {
    Command::new("del")
        .about("remove packages")
        .disable_help_subcommand(true)
        .arg(Arg::new("pkgs")
            .required(true)
            .action(ArgAction::Append)
            .value_name("PKG")
            .help("packages to remove"))
}

pub async fn run(args: &ArgMatches, client: &mut Client) -> Result<()> {
    let pkgs: Vec<String> = args
        .get_many::<String>("pkgs")
        .unwrap()
        .map(|s| s.to_string())
        .collect();
    let request = tonic::Request::new(ListRequest { data: pkgs });
    let response = client.remove_packages(request).await?;
    let mut stream = response.into_inner();
    while let Some(response) = stream.message().await? {
        println!("{}", response.data);
    }
    Ok(())
}
