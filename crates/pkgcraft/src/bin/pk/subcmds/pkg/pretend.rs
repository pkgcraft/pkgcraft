use std::io::stdin;
use std::process::ExitCode;

use anyhow::anyhow;
use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::pkg::BuildablePackage;
use pkgcraft::repo::{PkgRepository, Repo};
use pkgcraft::restrict::{self, Restrict};
use scallop::pool::Pool;

use crate::StdinArgs;

#[derive(Debug, Args)]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Target repository
    #[arg(short, long, default_value = "gentoo", required = false)]
    repo: String,

    // positionals
    /// Target packages
    #[arg(value_name = "PKG", required = false)]
    vals: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, config: &Config) -> anyhow::Result<ExitCode> {
        let mut restricts = vec![];

        if self.vals.stdin_args()? {
            for line in stdin().lines() {
                for s in line?.split_whitespace() {
                    restricts.push(restrict::parse::dep(s)?);
                }
            }
        } else {
            for s in &self.vals {
                restricts.push(restrict::parse::dep(s)?);
            }
        }

        // combine restricts into a single entity
        let restrict = Restrict::and(restricts);

        // determine target repo
        // TODO: use configured ebuild repos instead of raw ones
        // TODO: support binpkg repos
        let repo = match config.repos.get(&self.repo) {
            Some(r) => Ok(r.clone()),
            None => Repo::from_path(&self.repo, 0, &self.repo, true),
        };

        let repo = repo.map_err(|_| anyhow!("unknown repo: {}", self.repo))?;
        let repo = repo
            .as_ebuild()
            .ok_or_else(|| anyhow!("non-ebuild repo: {repo}"))?;

        let jobs = self.jobs.unwrap_or_else(num_cpus::get);
        let mut pool = Pool::new(jobs)?;

        // run pkg_pretend across selected pkgs
        // TODO: iterate over repo using non-sourced pkgs sourcing inside the forked processes
        for pkg in repo.iter_restrict(restrict) {
            pool.spawn(|| -> scallop::Result<()> { pkg.pretend() })?;
        }

        pool.join()?;

        Ok(ExitCode::SUCCESS)
    }
}
