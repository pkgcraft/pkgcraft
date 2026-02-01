use std::process::ExitCode;

use camino::Utf8PathBuf;
use clap::Args;
use clap::builder::{PossibleValuesParser, TypedValueParser};
use pkgcraft::cli::Targets;
use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::{Cache, CacheFormat};
use strum::VariantNames;

use super::repo_caches;

#[derive(Args)]
#[clap(next_help_heading = "Remove options")]
pub(crate) struct Command {
    /// Custom cache path
    #[arg(short, long)]
    path: Option<Utf8PathBuf>,

    /// Cache formats
    #[arg(
        short = 'F',
        long = "format",
        hide_possible_values = true,
        value_name = "FORMAT[,...]",
        value_delimiter = ',',
        value_parser = PossibleValuesParser::new(CacheFormat::VARIANTS)
            .map(|s| s.parse::<CacheFormat>().unwrap()),
    )]
    formats: Vec<CacheFormat>,

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

        for repo in &repos {
            for cache in repo_caches(repo, &self.formats, self.path.as_deref())? {
                cache.remove(repo)?;
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
