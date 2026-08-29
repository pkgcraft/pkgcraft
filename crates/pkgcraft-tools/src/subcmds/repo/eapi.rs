use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use indexmap::IndexMap;
use pkgcraft::cli::Targets;
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
        let repos = Targets::new(config)?
            .repo_targets(&self.repos)?
            .ebuild_repos()?;

        let mut failed = false;
        let mut stdout = io::stdout().lock();
        // indentation used when targeting multiple repos
        let indent = if repos.len() == 1 { "" } else { "  " };

        for repo in &repos {
            let mut eapis = IndexMap::<_, Vec<_>>::new();

            // TODO: use parallel iterator
            let mut iter = repo.iter_raw_ordered().log_errors(self.ignore);
            let mut repo_count = 0;
            for pkg in &mut iter {
                eapis.entry(pkg.eapi()).or_default().push(pkg.cpv().clone());
                repo_count += 1;
            }
            failed |= iter.failed();

            if let Some(eapi) = self.eapi {
                if let Some(cpvs) = eapis.get(eapi) {
                    for cpv in cpvs {
                        writeln!(stdout, "{cpv}")?;
                    }
                }
            } else if !eapis.is_empty() {
                // determine line title output justification
                let max_eapi_width: usize = eapis
                    .iter()
                    .map(|(id, _)| id.to_string().len())
                    .max()
                    .unwrap_or_default();
                let title_width = max_eapi_width + 5;

                // determine pkgs count output justification
                let max_pkgs_width: usize = eapis
                    .iter()
                    .map(|(_, cpvs)| cpvs.len().to_string().len())
                    .chain([repo_count.to_string().len()])
                    .max()
                    .unwrap_or_default();
                let pkgs_width = max_pkgs_width + 1;

                // output repo name when targeting multiple repos
                if repos.len() > 1 {
                    writeln!(stdout, "{repo}")?;
                }

                eapis.sort_keys();
                for (eapi, cpvs) in &eapis {
                    // determine the percentage of target EAPI pkgs in the repo
                    let pkgs_count = cpvs.len();
                    let proportion = pkgs_count as f64 / repo_count as f64;
                    let percent = proportion * 100.0;
                    let bar_width = 50;
                    let hash_count = (proportion * bar_width as f64).round() as usize;
                    let hashes = "#".repeat(hash_count);
                    let dashes = "-".repeat(bar_width - hash_count);
                    let percentage = format!("({percent:>5.2}%) [{hashes}{dashes}]");

                    // output EAPI pkgs count and percentage
                    let s = if pkgs_count > 1 { "s" } else { " " };
                    let pkgs_count = format!("{pkgs_count:>pkgs_width$} pkg{s}");
                    let title = format!("EAPI {eapi}");
                    writeln!(
                        stdout,
                        "{indent}{title:>title_width$}: {pkgs_count} {percentage}"
                    )?;
                }

                // output the total count of repo pkgs
                let title = "total".to_string();
                let s = if repo_count > 1 { "s" } else { "" };
                let pkgs_count = format!("{repo_count:>pkgs_width$} pkg{s}");
                writeln!(stdout, "{indent}{title:>title_width$}: {pkgs_count}")?;
            }
        }

        Ok(ExitCode::from(failed as u8))
    }
}
