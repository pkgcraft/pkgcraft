use crate::Client;

mod scan;
mod version;

#[derive(clap::Subcommand)]
pub(crate) enum Subcommand {
    /// queue a scanning run
    Scan(scan::Command),
    /// query for client/server version info
    Version(version::Command),
}

impl Subcommand {
    pub(super) async fn run(&self, client: &mut Client) -> anyhow::Result<()> {
        match self {
            Self::Scan(cmd) => cmd.run(client).await,
            Self::Version(cmd) => cmd.run(client).await,
        }
    }
}
