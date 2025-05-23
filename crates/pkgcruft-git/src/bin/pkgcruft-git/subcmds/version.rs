use crate::Client;
use pkgcruft_git::proto::EmptyRequest;

#[derive(clap::Args)]
pub(crate) struct Command;

impl Command {
    pub(super) async fn run(&self, client: &mut Client) -> anyhow::Result<()> {
        let request = tonic::Request::new(EmptyRequest {});
        let response = client.version(request).await?;
        let server_version = response.into_inner().data;
        let client_version =
            format!("{}-{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
        println!("client: {client_version}, server: {server_version}");
        Ok(())
    }
}
