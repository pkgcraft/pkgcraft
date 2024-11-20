use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::cache::{Cache, CacheFormat};

use crate::args::target_ebuild_repo;

#[derive(Debug, Args)]
pub(crate) struct Command {
    /// Custom cache path
    #[arg(short, long)]
    path: Option<String>,

    /// Custom cache format
    #[arg(long)]
    format: Option<CacheFormat>,

    // positionals
    /// Target repository
    #[arg(default_value = ".")]
    repo: String,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let repo = target_ebuild_repo(config, &self.repo)?;
        let format = self.format.unwrap_or(repo.metadata().cache().format());

        let cache = if let Some(path) = self.path.as_ref() {
            format.from_path(path)
        } else {
            format.from_repo(&repo)
        };

        cache.clean(&repo)?;

        Ok(ExitCode::SUCCESS)
    }
}
