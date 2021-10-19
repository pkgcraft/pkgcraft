use anyhow::Result;
use clap::{App, AppSettings};

use crate::Client;
use arcanist::proto::StringRequest;

#[rustfmt::skip]
pub fn cmd() -> App<'static> {
    App::new("version")
        .about("query arcanist for client/server version info")
        .setting(AppSettings::DisableHelpSubcommand)
}

pub async fn run(client: &mut Client) -> Result<()> {
    let version = format!("{}-{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
    let request = tonic::Request::new(StringRequest { data: version });
    let response = client.version(request).await?;
    println!("{}", response.into_inner().data);
    Ok(())
}
