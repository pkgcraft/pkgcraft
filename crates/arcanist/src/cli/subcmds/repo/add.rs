use anyhow::{Context, Result};
use clap::{Arg, ArgMatches, Command};

use crate::Client;
use arcanist::proto::AddRepoRequest;

#[rustfmt::skip]
pub fn cmd() -> Command {
    Command::new("add")
        .about("register repo")
        .arg(Arg::new("name")
            .required(true)
            .help("repo name"))
        .arg(Arg::new("uri")
            .required(true)
            .help("repo location"))
}

pub async fn run(args: &ArgMatches, client: &mut Client) -> Result<()> {
    let name = args.get_one::<String>("name").unwrap().to_string();
    let uri = args.get_one::<String>("uri").unwrap().to_string();
    let request = tonic::Request::new(AddRepoRequest {
        name: name.clone(),
        uri,
    });
    let response = client
        .add_repo(request)
        .await
        .context(format!("failed adding repo: {name}"))?;
    println!("{}", response.into_inner().data);
    Ok(())
}
