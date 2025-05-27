use std::io::{self, BufRead, IsTerminal};
use std::ops::Deref;

use anyhow::anyhow;
use itertools::Itertools;
use pkgcruft::report::Report;
use pkgcruft::reporter::{FancyReporter, Reporter};
use pkgcruft_git::git;
use pkgcruft_git::proto::PushRequest;

use crate::Client;

#[derive(clap::Args)]
pub(crate) struct Command {}

impl Command {
    pub(super) async fn run(&self, client: &mut Client) -> anyhow::Result<()> {
        let mut stdout = io::stdout().lock();
        let stdin = io::stdin().lock();
        let mut reporter: Reporter = FancyReporter::default().into();

        // pull object directories from the environment
        //
        // git2::Repository::open_from_env() doesn't appear to respect the temporary
        // object directory used for incoming objects before they're merged into the tree
        // so we manually add them ourselves.
        let mut odbs = vec![];
        if let Ok(value) = std::env::var("GIT_OBJECT_DIRECTORY") {
            odbs.push(value);
        }
        if let Ok(values) = std::env::var("GIT_ALTERNATE_OBJECT_DIRECTORIES") {
            odbs.extend(values.split(':').map(|s| s.to_string()));
        }

        // WARNING: This appears to invalidate the environment in some fashion so
        // std::env::var() calls don't work as expected after it even though
        // std::env::vars() will still show all the variables.
        //
        // open git repo specified by $GIT_DIR
        let repo = git2::Repository::open_from_env()
            .map_err(|e| anyhow!("failed opening git repo: {e}"))?;

        // manually add all object directories so incoming commits can be found
        for odb in odbs {
            repo.odb()?.add_disk_alternate(&odb)?;
        }

        if stdin.is_terminal() {
            anyhow::bail!("requires running as a git pre-receive hook");
        }

        let mut failed = false;
        for line in stdin.lines() {
            let line = line?;
            // TODO: skip pushes where the ref name doesn't match the default branch
            // get push information
            let Some((old_ref, new_ref, ref_name)) = line.split(' ').collect_tuple() else {
                anyhow::bail!("invalid pre-receive hook arguments: {line}");
            };

            // generate patch data
            let mut data = vec![];
            let diff = git::diff(&repo, old_ref, new_ref)?;
            for (idx, _delta) in diff.deltas().enumerate() {
                if let Ok(Some(mut patch)) = git2::Patch::from_diff(&diff, idx) {
                    let buf = patch.to_buf()?;
                    data.extend(buf.deref());
                }
            }

            // send request to server
            let push = PushRequest {
                old_ref: old_ref.to_string(),
                new_ref: new_ref.to_string(),
                ref_name: ref_name.to_string(),
                patch: data,
            };
            let request = tonic::Request::new(push);
            let response = client.push(request).await?;
            let response = response.into_inner();
            failed |= response.failed;

            // output reports
            for report in response.reports {
                let report = Report::from_json(&report)?;
                reporter.report(&report, &mut stdout)?;
            }
        }

        if failed {
            anyhow::bail!("scanning errors found")
        } else {
            Ok(())
        }
    }
}
