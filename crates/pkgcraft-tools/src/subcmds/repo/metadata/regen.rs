use std::io::{IsTerminal, stdout};
use std::process::ExitCode;

use camino::Utf8PathBuf;
use clap::Args;
use clap::builder::{PossibleValuesParser, TypedValueParser};
use pkgcraft::cli::Targets;
use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::CacheFormat;
use pkgcraft::utils::bounded_thread_pool;
use strum::VariantNames;

use super::repo_caches;

#[derive(Args)]
#[clap(next_help_heading = "Regen options")]
pub(crate) struct Command {
    /// Parallel jobs to run
    #[arg(short, long, default_value_t = num_cpus::get())]
    jobs: usize,

    /// Force regeneration to occur
    #[arg(short, long)]
    force: bool,

    /// Custom cache path
    #[arg(short, long)]
    path: Option<Utf8PathBuf>,

    /// Disable progress bar
    #[arg(short, long)]
    no_progress: bool,

    /// Capture stderr and stdout
    #[arg(short, long)]
    output: bool,

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

    /// Update local USE cache
    #[arg(long)]
    use_local: bool,

    // positionals
    /// Target repositories
    #[arg(value_name = "REPO", default_value = ".", help_heading = "Arguments")]
    repos: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // build custom, global thread pool when limiting jobs
        bounded_thread_pool(self.jobs);

        let repos = Targets::new(config)?
            .repo_targets(&self.repos)?
            .ebuild_repos()?;

        for repo in &repos {
            for cache in repo_caches(repo, &self.formats, self.path.as_deref())? {
                cache
                    .regen(repo)
                    .force(self.force)
                    .progress(stdout().is_terminal() && !self.no_progress)
                    .output(self.output)
                    .run()?;

                if self.use_local {
                    repo.metadata().use_local_update(repo)?;
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
