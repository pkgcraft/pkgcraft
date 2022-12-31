use anyhow::Result;
use clap::Command;

use crate::Client;
use arcanist::proto::StringRequest;

#[rustfmt::skip]
pub fn cmd() -> Command {
    Command::new("version")
        .about("query arcanist for client/server version info")
        .disable_help_subcommand(true)
}

pub async fn run(client: &mut Client) -> Result<()> {
    let version = format!("{}-{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
    let request = tonic::Request::new(StringRequest { data: version });
    let response = client.version(request).await?;
    println!("{}", response.into_inner().data);
    Ok(())
}
