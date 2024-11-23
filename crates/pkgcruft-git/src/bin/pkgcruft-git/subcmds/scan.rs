use std::io;

use pkgcruft::report::Report;
use pkgcruft::reporter::{FancyReporter, Reporter};
use pkgcruft_git::proto::StringRequest;

use crate::Client;

#[derive(clap::Args)]
pub(crate) struct Command {}

impl Command {
    pub(super) async fn run(&self, client: &mut Client) -> anyhow::Result<()> {
        let mut stdout = io::stdout().lock();
        let mut reporter: Reporter = FancyReporter::default().into();

        // TODO: send git patch data for request
        let request = tonic::Request::new(StringRequest { data: Default::default() });
        let response = client.scan(request).await?;

        // output report stream
        let mut stream = response.into_inner();
        while let Some(response) = stream.message().await? {
            let report = Report::from_json(&response.data)?;
            reporter.report(&report, &mut stdout)?;
        }

        Ok(())
    }
}
