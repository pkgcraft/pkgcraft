use std::process::ExitCode;

use camino::Utf8PathBuf;
use clap::Args;
use pkgcraft::cli::Targets;
use pkgcraft::config::Config;

#[derive(Args)]
#[clap(next_help_heading = "Revdeps options")]
pub(crate) struct Command {
    /// Ignore invalid packages
    #[arg(short, long)]
    ignore: bool,

    /// Target repository
    #[arg(short, long, default_value = ".")]
    repo: String,

    // positionals
    /// Target path
    #[arg(default_value = ".")]
    path: Utf8PathBuf,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let repo = Targets::new(config)
            .finalize_repos([&self.repo])?
            .ebuild_repo()?;

        // TODO: load/update revdeps cache instead of creating
        let cache = repo.revdeps(self.ignore)?;

        // serialize cache to disk in qa reports format
        cache.serialize_to_qa(&self.path)?;

        Ok(ExitCode::SUCCESS)
    }
}
