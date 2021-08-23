use anyhow::Result;
use clap::{App, AppSettings, ArgMatches};

use crate::arcanist::ArcanistRequest;
use crate::settings::Settings;
use crate::Client;

pub fn cmd() -> App<'static> {
    App::new("version")
        .about("query arcanist for client/server version info")
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DisableVersionForSubcommands)
}

pub async fn run(_args: &ArgMatches, client: &mut Client, _settings: &mut Settings) -> Result<()> {
    let version = format!("{}-{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    let request = tonic::Request::new(ArcanistRequest { message: version });
    let response = client.version(request).await?;
    let resp = response.into_inner();
    println!("{}", resp.message);
    Ok(())
}
