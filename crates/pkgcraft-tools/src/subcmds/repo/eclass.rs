use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use indexmap::IndexMap;
use pkgcraft::cli::Targets;
use pkgcraft::config::Config;
use pkgcraft::pkg::Package;
use pkgcraft::traits::LogErrors;

#[derive(Args)]
#[clap(next_help_heading = "Eclass options")]
pub(crate) struct Command {
    /// Output packages for a target eclass
    #[arg(long)]
    eclass: Option<String>,

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
        for repo in &repos {
            // fail fast for nonexistent eclass selection
            let selected = if let Some(name) = self.eclass.as_deref() {
                let eclass = repo
                    .eclasses()
                    .get(name)
                    .ok_or_else(|| anyhow::anyhow!("unknown eclass: {name}"))?;
                Some(eclass)
            } else {
                None
            };

            let mut eclasses = IndexMap::<_, Vec<_>>::new();

            // TODO: use parallel iterator
            let mut iter = repo.iter_ordered().log_errors(self.ignore);
            for pkg in &mut iter {
                let cpv = pkg.cpv();
                for eclass in pkg.inherited() {
                    eclasses
                        .entry(eclass.clone())
                        .or_default()
                        .push(cpv.clone());
                }
            }
            failed |= iter.failed();

            if let Some(eclass) = selected {
                // ouput all packages using a selected eclass
                if let Some(cpvs) = eclasses.get(eclass) {
                    for cpv in cpvs {
                        writeln!(stdout, "{cpv}")?;
                    }
                }
            } else if !eclasses.is_empty() {
                // ouput all eclasses with the number of packages that use them
                writeln!(stdout, "{repo}")?;
                eclasses.par_sort_by(|_k1, v1, _k2, v2| v1.len().cmp(&v2.len()));
                for (eclass, cpvs) in &eclasses {
                    let count = cpvs.len();
                    let s = if count != 1 { "s" } else { "" };
                    writeln!(stdout, "  {eclass}: {count} pkg{s}")?;
                }
            }
        }

        Ok(ExitCode::from(failed as u8))
    }
}
