use anyhow::Result;

use crate::Client;
use pkgcruft_git::proto::StringRequest;

#[derive(clap::Args)]
pub(crate) struct Command;

impl Command {
    pub(super) async fn run(&self, client: &mut Client) -> Result<()> {
        let version = format!("{}-{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
        let request = tonic::Request::new(StringRequest { data: version });
        let response = client.version(request).await?;
        println!("{}", response.into_inner().data);
        Ok(())
    }
}
