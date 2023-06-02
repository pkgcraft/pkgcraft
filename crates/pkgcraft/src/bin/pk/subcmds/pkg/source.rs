use std::io::stdin;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::pkg::ebuild::RawPkg;
use pkgcraft::pkg::SourceablePackage;
use pkgcraft::repo::Repo;
use pkgcraft::restrict::{self, Restrict};
use scallop::pool::PoolIter;
use tracing::error;

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
        let repo = match config.repos.get(&self.repo) {
            Some(r) => Ok(r.clone()),
            None => Repo::from_path(&self.repo, 0, &self.repo, true),
        };

        let repo = repo.map_err(|_| anyhow!("unknown repo: {}", self.repo))?;
        let repo = repo
            .as_ebuild()
            .ok_or_else(|| anyhow!("non-ebuild repo: {repo}"))?;

        let jobs = self.jobs.unwrap_or_else(num_cpus::get);
        let pkgs = repo.iter_raw_restrict(restrict);
        let func = |pkg: RawPkg| -> scallop::Result<(String, Duration)> {
            let start = Instant::now();
            pkg.source()?;
            let elapsed = start.elapsed();
            Ok((pkg.to_string(), elapsed))
        };

        let mut errors = 0;
        for r in PoolIter::new(jobs, pkgs, func)? {
            match r {
                Ok((pkg, elapsed)) => println!("{pkg}: {elapsed:?}"),
                Err(e) => {
                    // log errors
                    errors += 1;
                    error!("{e}");
                }
            }
        }

        if errors == 0 {
            Ok(ExitCode::SUCCESS)
        } else {
            Ok(ExitCode::FAILURE)
        }
    }
}
