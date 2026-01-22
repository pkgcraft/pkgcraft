use std::io::Write;
use std::process::ExitCode;

use clap::Args;
use itertools::Itertools;
use pkgcraft::cli::Targets;
use pkgcraft::config::Config;
use pkgcraft::restrict::Scope;
use pkgcruft::check::Check;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Check options")]
pub(super) struct Subcommand {
    /// Output extended information
    #[arg(short, long)]
    info: bool,

    /// Target repo
    #[arg(long, num_args = 0..=1, default_missing_value = ".")]
    repo: Option<String>,
}

impl Subcommand {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let checks: Vec<_> = if let Some(repo) = self.repo.as_deref() {
            let mut config = Config::new("pkgcraft", "");
            let repo = Targets::new(&mut config)?
                .repo_targets([repo])?
                .ebuild_repo()?;
            Check::iter_supported(&repo, Scope::Repo).collect()
        } else {
            Check::iter().collect()
        };

        let mut stdout = anstream::stdout().lock();
        for check in checks {
            writeln!(stdout, "{check}")?;
            if self.info {
                if !check.context.is_empty() {
                    let contexts = check.context.iter().join(", ");
                    writeln!(stdout, "  context: {contexts}")?;
                }
                let reports = check.reports.iter().join(", ");
                writeln!(stdout, "  reports: {reports}\n")?;
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
