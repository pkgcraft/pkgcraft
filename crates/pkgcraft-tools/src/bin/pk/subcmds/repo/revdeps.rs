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

    /// Target directory
    #[arg(short, long, default_value = ".")]
    dir: Utf8PathBuf,

    // positionals
    /// Target repository
    #[arg(default_value = ".")]
    repo: String,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let repo = Targets::new(config)
            .finalize_repos([&self.repo])?
            .ebuild_repo()?;

        // TODO: load/update revdeps cache instead of creating
        let cache = repo.revdeps(self.ignore)?;

        // serialize cache to disk in qa reports format
        cache.serialize_to_qa(&self.dir)?;

        Ok(ExitCode::SUCCESS)
    }
}
