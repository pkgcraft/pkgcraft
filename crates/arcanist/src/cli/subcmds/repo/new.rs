use anyhow::{Context, Result};
use clap::{Arg, ArgMatches, Command};

use crate::Client;
use arcanist::proto::StringRequest;

#[rustfmt::skip]
pub fn cmd() -> Command {
    Command::new("new")
        .about("create repo")
        .arg(Arg::new("name")
            .required(true)
            .help("repo name"))
}

pub async fn run(args: &ArgMatches, client: &mut Client) -> Result<()> {
    let name = args.get_one::<String>("name").unwrap().to_string();
    let request = tonic::Request::new(StringRequest { data: name.clone() });
    client
        .create_repo(request)
        .await
        .context(format!("failed creating repo: {name}"))?;
    Ok(())
}
