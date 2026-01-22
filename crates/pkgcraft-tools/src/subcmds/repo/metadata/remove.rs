use std::process::ExitCode;

use clap::Args;
use pkgcraft::cli::Targets;
use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::{Cache, CacheFormat};

#[derive(Args)]
#[clap(next_help_heading = "Remove options")]
pub(crate) struct Command {
    /// Custom cache path
    #[arg(short, long)]
    path: Option<String>,

    /// Custom cache format
    #[arg(long)]
    format: Option<CacheFormat>,

    // positionals
    /// Target repository
    #[arg(default_value = ".", help_heading = "Arguments")]
    repo: String,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let repo = Targets::new(config)?
            .repo_targets([&self.repo])?
            .ebuild_repo()?;
        let format = self.format.unwrap_or(repo.metadata().cache().format());

        let cache = if let Some(path) = self.path.as_ref() {
            format.from_path(path)
        } else {
            format.from_repo(&repo)
        };

        cache.remove(&repo)?;

        Ok(ExitCode::SUCCESS)
    }
}
