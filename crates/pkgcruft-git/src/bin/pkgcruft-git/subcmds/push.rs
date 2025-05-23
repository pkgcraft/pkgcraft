use std::io::{self, BufRead, IsTerminal};

use pkgcruft::report::Report;
use pkgcruft::reporter::{FancyReporter, Reporter};
use pkgcruft_git::proto::PushRequest;

use crate::Client;

#[derive(clap::Args)]
pub(crate) struct Command {}

impl Command {
    pub(super) async fn run(&self, client: &mut Client) -> anyhow::Result<()> {
        let mut stdout = io::stdout().lock();
        let stdin = io::stdin().lock();
        let mut reporter: Reporter = FancyReporter::default().into();

        if stdin.is_terminal() {
            anyhow::bail!("requires running as a git pre-receive hook");
        }

        for line in stdin.lines() {
            let push: PushRequest = line?.parse()?;
            let request = tonic::Request::new(push);
            let response = client.push(request).await?;

            // output report stream
            let mut stream = response.into_inner();
            while let Some(response) = stream.message().await? {
                let report = Report::from_json(&response.data)?;
                reporter.report(&report, &mut stdout)?;
            }
        }

        Ok(())
    }
}
