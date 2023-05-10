use std::io::stdin;
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::{Config, RepoSetType};
use pkgcraft::pkg::BuildablePackage;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::{self, Restrict};

use crate::{Run, StdinArgs};

#[derive(Debug, Args)]
pub struct Command {
    vals: Vec<String>,
}

impl Run for Command {
    fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
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

        // pull all ebuild repos into a set
        // TODO: use configured ebuild repos instead of raw ones
        let ebuild_repos = config.repos.set(RepoSetType::Ebuild);

        // run pkg_pretend across selected pkgs
        // TODO: run pkg_pretend in parallel for pkgs
        for pkg in ebuild_repos.iter_restrict(restrict) {
            // TODO: internally unwrap pkg types during iteration
            let (pkg, _) = pkg.as_ebuild().unwrap();
            if let Err(e) = pkg.pretend() {
                eprintln!("{pkg}: {e}");
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
