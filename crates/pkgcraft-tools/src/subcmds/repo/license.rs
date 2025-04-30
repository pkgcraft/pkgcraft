use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use pkgcraft::cli::Targets;
use pkgcraft::config::Config;
use pkgcraft::pkg::Package;
use pkgcraft::traits::LogErrors;

#[derive(Args)]
#[clap(next_help_heading = "License options")]
pub(crate) struct Command {
    /// Output packages for target licenses
    #[arg(long, value_name = "LICENSE", value_delimiter = ',')]
    licenses: Vec<String>,

    /// Ignore invalid packages
    #[arg(short, long)]
    ignore: bool,

    // positionals
    /// Target repositories
    #[arg(value_name = "REPO", default_value = ".", help_heading = "Arguments")]
    repos: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let repos = Targets::new(config)
            .repo_targets(&self.repos)?
            .ebuild_repos()?;

        let mut failed = false;
        let mut stdout = io::stdout().lock();
        for repo in &repos {
            // fail fast for nonexistent license selection
            let selected: IndexSet<_> = if !self.licenses.is_empty() {
                self.licenses
                    .iter()
                    .map(|x| {
                        repo.licenses()
                            .get(x)
                            .ok_or_else(|| anyhow::anyhow!("unknown license: {x}"))
                    })
                    .try_collect()?
            } else {
                Default::default()
            };

            let mut licenses = IndexMap::<_, IndexSet<_>>::new();

            // TODO: use parallel iterator
            let mut iter = repo.iter_ordered().log_errors(self.ignore);
            for pkg in &mut iter {
                let cpv = pkg.cpv();
                for license in pkg.license().iter_flatten() {
                    licenses
                        .entry(license.clone())
                        .or_default()
                        .insert(cpv.clone());
                }
            }
            failed |= iter.failed();

            if !selected.is_empty() {
                // ouput all packages using selected licenses
                let mut cpvs: IndexSet<_> = selected
                    .iter()
                    .filter_map(|x| licenses.get(x.as_str()))
                    .flatten()
                    .collect();
                cpvs.sort();
                for cpv in cpvs {
                    writeln!(stdout, "{cpv}")?;
                }
            } else if !licenses.is_empty() {
                // ouput all licenses with the number of packages that use them
                writeln!(stdout, "{repo}")?;
                licenses.par_sort_by(|_k1, v1, _k2, v2| v1.len().cmp(&v2.len()));
                for (license, cpvs) in &licenses {
                    let count = cpvs.len();
                    let s = if count != 1 { "s" } else { "" };
                    writeln!(stdout, "  {license}: {count} pkg{s}")?;
                }
            }
        }

        Ok(ExitCode::from(failed as u8))
    }
}
