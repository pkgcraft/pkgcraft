use anyhow::{Context, Result};
use clap::{App, Arg, ArgMatches, ArgSettings};

use crate::Client;
use arcanist::proto::StringRequest;

#[rustfmt::skip]
pub fn cmd() -> App<'static> {
    App::new("new")
        .about("create repo")
        .arg(Arg::new("name")
            .setting(ArgSettings::Required)
            .help("repo name"))
}

pub async fn run(args: &ArgMatches, client: &mut Client) -> Result<()> {
    let name = args.value_of("name").unwrap().to_string();
    let request = tonic::Request::new(StringRequest { data: name.clone() });
    client
        .create_repo(request)
        .await
        .context(format!("failed creating repo: {:?}", &name))?;
    Ok(())
}
