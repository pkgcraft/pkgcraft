use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use indexmap::IndexMap;
use itertools::Itertools;
use pkgcraft::cli::target_ebuild_repo;
use pkgcraft::config::Config;
use pkgcraft::pkg::Package;
use pkgcraft::traits::LogErrors;

#[derive(Args)]
#[clap(next_help_heading = "License options")]
pub(crate) struct Command {
    /// Output packages for a target license
    #[arg(long)]
    license: Option<String>,

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
            // fail fast for nonexistent license selection
            let selected = if let Some(name) = self.license.as_deref() {
                let license = repo
                    .licenses()
                    .get(name)
                    .ok_or_else(|| anyhow::anyhow!("unknown license: {name}"))?;
                Some(license)
            } else {
                None
            };

            let mut licenses = IndexMap::<_, Vec<_>>::new();

            // TODO: use parallel iterator
            let mut iter = repo.iter_ordered().log_errors();
            for pkg in &mut iter {
                let cpv = pkg.cpv();
                for license in pkg.license().iter_flatten() {
                    licenses
                        .entry(license.clone())
                        .or_default()
                        .push(cpv.clone());
                }
            }
            failed |= iter.failed();

            if let Some(license) = selected {
                // ouput all packages using a selected license
                if let Some(cpvs) = licenses.get(license) {
                    for cpv in cpvs {
                        writeln!(stdout, "{cpv}")?;
                    }
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
