use std::io::{stdout, IsTerminal};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::cli::{MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::cache::{Cache, CacheFormat};
use pkgcraft::repo::{PkgRepository, RepoFormat};
use pkgcraft::restrict::Restrict;

#[derive(Debug, Args)]
pub(crate) struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Force regeneration to occur
    #[arg(short, long)]
    force: bool,

    /// Verify metadata without updating cache
    #[arg(short = 'V', long)]
    verify: bool,

    /// Custom cache path
    #[arg(short, long)]
    path: Option<String>,

    /// Disable progress bar
    #[arg(short, long)]
    no_progress: bool,

    /// Allow output to stderr and stdout (suppressed by default)
    #[arg(short, long)]
    output: bool,

    /// Custom cache format
    #[arg(long)]
    format: Option<CacheFormat>,

    // positionals
    /// Target packages or paths
    #[arg(value_name = "TARGET", default_value = ".")]
    targets: Vec<MaybeStdinVec<String>>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // determine target restrictions
        let targets = TargetRestrictions::new(config)
            .repo_format(RepoFormat::Ebuild)
            .targets(self.targets.iter().flatten())?;

        for (repo_set, restricts) in targets {
            for restrict in restricts {
                for repo in repo_set.ebuild() {
                    let format = self.format.unwrap_or(repo.metadata().cache().format());

                    let cache = if let Some(path) = self.path.as_ref() {
                        format.from_path(path)
                    } else {
                        format.from_repo(repo)
                    };

                    let mut regen = cache.regen();
                    regen
                        .jobs(self.jobs.unwrap_or_default())
                        .force(self.force)
                        .progress(stdout().is_terminal() && !self.no_progress && !self.output)
                        .output(self.output)
                        .verify(self.verify);

                    // TODO: use parallel Cpv restriction iterator
                    // skip repo level targets that needlessly slow down regen
                    if restrict != Restrict::True {
                        regen.targets(repo.iter_cpv_restrict(&restrict));
                    }

                    regen.run(repo)?;
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
