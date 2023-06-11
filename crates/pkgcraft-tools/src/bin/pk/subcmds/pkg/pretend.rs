use std::io::stdin;
use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::{Config, RepoSetType};
use pkgcraft::pkg::ebuild::{Pkg, RawPkg};
use pkgcraft::pkg::BuildablePackage;
use pkgcraft::repo::set::RepoSet;
use pkgcraft::repo::RepoFormat;
use pkgcraft::restrict::{self, Restrict};
use scallop::pool::PoolIter;

use crate::args::bounded_jobs;
use crate::StdinArgs;

#[derive(Debug, Args)]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Target repository
    #[arg(short, long, required = false)]
    repo: Option<String>,

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

        // determine target repo set
        let reposet = if let Some(repo) = self.repo.as_ref() {
            let repo = if let Some(r) = config.repos.get(repo) {
                Ok(r.clone())
            } else if Path::new(repo).exists() {
                RepoFormat::Ebuild.load_from_path(repo, 0, repo, true)
            } else {
                anyhow::bail!("unknown repo: {repo}")
            }?;
            RepoSet::new([&repo])
        } else {
            config.repos.set(RepoSetType::Ebuild)
        };

        // TODO: use configured ebuild repos instead of raw ones
        // TODO: support binpkg repos
        // restrict searches to ebuild repos
        let repos: Vec<_> = reposet
            .repos()
            .iter()
            .filter_map(|r| r.as_ebuild())
            .collect();
        if repos.is_empty() {
            anyhow::bail!("no matching ebuild repos found");
        }

        // convert repos into packages
        let pkgs = repos
            .into_iter()
            .flat_map(|r| r.iter_raw_restrict(restrict.clone()));

        let mut failed = false;
        let jobs = bounded_jobs(self.jobs)?;
        let func = |raw_pkg: RawPkg| -> scallop::Result<()> {
            let pkg: Pkg = raw_pkg.into_pkg()?;
            pkg.pretend()
        };

        // run pkg_pretend across selected pkgs
        for r in PoolIter::new(jobs, pkgs, func, true)? {
            if let Err(e) = r {
                failed = true;
                eprintln!("{e}");
            }
        }

        if failed {
            Ok(ExitCode::FAILURE)
        } else {
            Ok(ExitCode::SUCCESS)
        }
    }
}
