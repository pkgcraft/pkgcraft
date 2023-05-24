use std::process::ExitCode;

use anyhow::anyhow;
use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::pkg::BuildablePackage;
use pkgcraft::repo::{PkgRepository, Repo};
use pkgcraft::restrict::{self, Restrict};
use scallop::pool::Pool;

use crate::Run;

#[derive(Debug, Args)]
pub struct Command {
    /// Target repository
    #[arg(short, long, default_value = "gentoo")]
    repo: String,

    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    // positionals
    /// Target packages
    #[arg(value_name = "PKG", default_value = "*", required = false)]
    vals: Vec<String>,
}

impl Run for Command {
    fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
        let mut restricts = vec![];

        for s in &self.vals {
            restricts.push(restrict::parse::dep(s)?);
        }

        // combine restricts into a single entity
        let restrict = Restrict::and(restricts);

        // determine target repo
        let repo = match config.repos.get(&self.repo) {
            Some(r) => Ok(r.clone()),
            None => Repo::from_path(&self.repo, 0, &self.repo, true),
        };

        // collapse repo into ebuild repo
        let repo = repo.map_err(|_| anyhow!("unknown repo: {}", self.repo))?;
        let repo = repo.as_ebuild().ok_or_else(|| anyhow!("non-ebuild repo: {}", self.repo))?;

        let jobs = self.jobs.unwrap_or_else(num_cpus::get);
        let mut pool = Pool::new(jobs)?;

        // generate metadata for the selected pkgs
        // TODO: iterate over repo using non-sourced pkgs sourcing inside the forked processes
        for pkg in repo.iter_restrict(restrict) {
            pool.spawn(|| {
                if let Err(e) = pkg.metadata() {
                    eprintln!("{pkg}: {e}");
                }
            })?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
