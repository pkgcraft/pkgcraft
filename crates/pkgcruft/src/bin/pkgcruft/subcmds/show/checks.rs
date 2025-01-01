use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::cli::target_ebuild_repo;
use pkgcraft::config::Config;
use pkgcruft::check::Check;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Check options")]
pub(super) struct Subcommand {
    /// Target repo
    #[arg(long, num_args = 0..=1, default_missing_value = ".")]
    repo: Option<String>,
}

impl Subcommand {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let checks: Vec<_> = if let Some(value) = self.repo.as_deref() {
            let mut config = Config::new("pkgcraft", "");
            let repo = target_ebuild_repo(&mut config, value)?;
            Check::iter_supported(&repo).collect()
        } else {
            Check::iter().collect()
        };

        let mut stdout = io::stdout().lock();
        for check in checks {
            writeln!(stdout, "{check}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
