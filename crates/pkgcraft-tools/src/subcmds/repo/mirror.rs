use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use indexmap::{IndexMap, IndexSet};
use pkgcraft::cli::Targets;
use pkgcraft::config::Config;
use pkgcraft::pkg::Package;
use pkgcraft::traits::LogErrors;

#[derive(Args)]
#[clap(next_help_heading = "Mirror options")]
pub(crate) struct Command {
    /// Output packages for a target mirror
    #[arg(long)]
    mirror: Option<String>,

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
            // fail fast for nonexistent mirror selection
            let selected = if let Some(name) = self.mirror.as_deref() {
                let mirror = repo
                    .mirrors()
                    .get(name)
                    .and_then(|mirrors| mirrors.first().map(|x| x.name()))
                    .ok_or_else(|| anyhow::anyhow!("unknown mirror: {name}"))?;
                Some(mirror)
            } else {
                None
            };

            let mut mirrors = IndexMap::<_, IndexSet<_>>::new();

            // TODO: use parallel iterator
            let mut iter = repo.iter_ordered().log_errors(self.ignore);
            for pkg in &mut iter {
                let cpv = pkg.cpv();
                for f in pkg.fetchables(false, false).filter_map(Result::ok) {
                    for mirror in f.mirrors().iter().map(|x| x.name()) {
                        mirrors
                            .entry(mirror.to_string())
                            .or_default()
                            .insert(cpv.clone());
                    }
                }
            }
            failed |= iter.failed();

            if let Some(mirror) = selected {
                // ouput all packages using a selected mirror
                if let Some(cpvs) = mirrors.get(mirror) {
                    for cpv in cpvs {
                        writeln!(stdout, "{cpv}")?;
                    }
                }
            } else if !mirrors.is_empty() {
                // ouput all mirrors with the number of packages that use them
                writeln!(stdout, "{repo}")?;
                mirrors.par_sort_by(|_k1, v1, _k2, v2| v1.len().cmp(&v2.len()));
                for (mirror, cpvs) in &mirrors {
                    let count = cpvs.len();
                    let s = if count != 1 { "s" } else { "" };
                    writeln!(stdout, "  {mirror}: {count} pkg{s}")?;
                }
            }
        }

        Ok(ExitCode::from(failed as u8))
    }
}
