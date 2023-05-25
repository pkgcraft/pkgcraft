use std::process::ExitCode;

use anyhow::anyhow;
use clap::Args;
use pkgcraft::pkg::BuildablePackage;
use pkgcraft::repo::{PkgRepository, Repo};
use pkgcraft::restrict::{self, Restrict};
use scallop::pool::Pool;

#[derive(Debug, Args)]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    // positionals
    /// Target packages
    #[arg(value_name = "PKG", default_value = "*", required = false)]
    vals: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, repo: Repo) -> anyhow::Result<ExitCode> {
        let mut restricts = vec![];

        for s in &self.vals {
            restricts.push(restrict::parse::dep(s)?);
        }

        // combine restricts into a single entity
        let restrict = Restrict::and(restricts);

        // collapse repo into ebuild repo
        let repo = repo
            .as_ebuild()
            .ok_or_else(|| anyhow!("non-ebuild repo: {repo}"))?;

        let jobs = self.jobs.unwrap_or_else(num_cpus::get);
        let mut pool = Pool::new(jobs)?;

        // generate metadata for the selected pkgs
        // TODO: iterate over repo using non-sourced pkgs sourcing inside the forked processes
        for pkg in repo.iter_restrict(restrict) {
            pool.spawn(|| pkg.metadata())?;
        }

        pool.join()?;

        Ok(ExitCode::SUCCESS)
    }
}
