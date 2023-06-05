use std::io::stdin;
use std::process::ExitCode;

use anyhow::anyhow;
use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::pkg::{ebuild::Pkg, BuildablePackage};
use pkgcraft::repo::Repo;
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
        let handle_error = |r: scallop::Result<()>, errors: &mut usize| {
            if let Err(e) = r {
                *errors += 1;
                eprintln!("{e}");
            }
        };
        let mut pool = Pool::new(jobs, handle_error)?;

        // run pkg_pretend across selected pkgs
        for raw_pkg in repo.iter_raw_restrict(restrict) {
            pool.spawn(move || {
                let pkg: Pkg = raw_pkg.into_pkg()?;
                pkg.pretend()
            })?;
        }

        let errors = pool.join()?;

        if errors == 0 {
            Ok(ExitCode::SUCCESS)
        } else {
            Ok(ExitCode::FAILURE)
        }
    }
}
