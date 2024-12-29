use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use indexmap::IndexMap;
use itertools::Itertools;
use pkgcraft::cli::target_ebuild_repo;
use pkgcraft::config::Config;
use pkgcraft::eapi::Eapi;
use pkgcraft::pkg::Package;
use pkgcraft::traits::LogErrors;

#[derive(Args)]
#[clap(next_help_heading = "Eapi options")]
pub(crate) struct Command {
    /// Output packages for a target EAPI
    #[arg(long)]
    eapi: Option<&'static Eapi>,

    // positionals
    /// Target repositories
    #[arg(value_name = "REPO", default_value = ".", help_heading = "Arguments")]
    repos: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let repos: Vec<_> = self
            .repos
            .iter()
            .map(|x| target_ebuild_repo(config, x))
            .try_collect()?;
        config.finalize()?;

        let mut failed = false;
        let mut stdout = io::stdout().lock();
        for repo in &repos {
            let mut eapis = IndexMap::<_, Vec<_>>::new();

            // TODO: use parallel iterator
            let mut iter = repo.iter_raw_ordered().log_errors();
            for pkg in &mut iter {
                eapis.entry(pkg.eapi()).or_default().push(pkg.cpv().clone());
            }
            failed |= iter.failed();

            if let Some(eapi) = self.eapi {
                if let Some(cpvs) = eapis.get(eapi) {
                    for cpv in cpvs {
                        writeln!(stdout, "{cpv}")?;
                    }
                }
            } else if !eapis.is_empty() {
                writeln!(stdout, "{repo}")?;
                eapis.sort_keys();
                for (eapi, cpvs) in &eapis {
                    let count = cpvs.len();
                    let s = if count != 1 { "s" } else { "" };
                    writeln!(stdout, "  EAPI {eapi}: {count} pkg{s}")?;
                }
            }
        }

        Ok(ExitCode::from(failed as u8))
    }
}
