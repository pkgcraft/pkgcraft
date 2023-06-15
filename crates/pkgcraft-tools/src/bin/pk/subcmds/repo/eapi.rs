use std::collections::HashMap;
use std::io::{stdout, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use itertools::Itertools;
use pkgcraft::config::Config;
use pkgcraft::dep::Cpv;
use pkgcraft::eapi::{Eapi, EAPIS};
use pkgcraft::pkg::Package;
use pkgcraft::repo::RepoFormat;

#[derive(Debug, Args)]
pub struct Command {
    // positionals
    /// Target repositories
    #[arg(value_name = "REPO", required = true)]
    repos: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, config: &Config) -> anyhow::Result<ExitCode> {
        // determine target repos
        let mut invalid = vec![];
        let mut repos = vec![];
        for repo in &self.repos {
            let repo = if let Some(r) = config.repos.get(repo) {
                Ok(r.clone())
            } else if Path::new(repo).exists() {
                RepoFormat::Ebuild.load_from_path(repo, 0, repo, true)
            } else {
                anyhow::bail!("unknown repo: {repo}")
            }?;

            if let Some(r) = repo.as_ebuild() {
                repos.push(r.clone());
            } else {
                invalid.push(repo);
            }
        }

        if !invalid.is_empty() {
            let repos = invalid.iter().map(|s| s.to_string()).join(", ");
            anyhow::bail!("non-ebuild repos: {repos}");
        }

        for repo in &repos {
            let mut eapis = HashMap::<&'static Eapi, Vec<Cpv>>::new();
            // TODO: use parallel iterator
            for pkg in repo.iter_raw() {
                eapis
                    .entry(pkg.eapi())
                    .or_insert_with(Vec::new)
                    .push(pkg.cpv().clone());
            }

            writeln!(stdout(), "{repo}:")?;
            for eapi in EAPIS.iter() {
                if let Some(cpvs) = eapis.get(eapi) {
                    writeln!(stdout(), "  EAPI {eapi}: {} pkgs", cpvs.len())?;
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
