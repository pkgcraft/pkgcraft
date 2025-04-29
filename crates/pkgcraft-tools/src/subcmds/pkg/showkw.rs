use std::io::{self, Write};
use std::process::ExitCode;

use clap::{Args, builder::ArgPredicate};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::cli::{MaybeStdinVec, Targets, TriState};
use pkgcraft::config::Config;
use pkgcraft::pkg::RepoPackage;
use pkgcraft::pkg::ebuild::keyword::Arch;
use pkgcraft::repo::RepoFormat;
use pkgcraft::traits::LogErrors;

#[derive(Args)]
#[clap(next_help_heading = "Target options")]
pub(crate) struct Command {
    /// Ignore invalid packages
    #[arg(short, long)]
    ignore: bool,

    /// Target repo
    #[arg(short, long)]
    repo: Option<String>,

    /// Target arches
    #[arg(
        short,
        long,
        value_name = "TARGET[,...]",
        value_delimiter = ',',
        allow_hyphen_values = true
    )]
    arches: Vec<TriState<Arch>>,

    /// Show prefix arches
    #[arg(short, long)]
    prefix: bool,

    // positionals
    /// Target packages or paths
    #[arg(
        // default to the current working directory
        default_value = ".",
        // default to all packages when targeting a repo
        default_value_if("repo", ArgPredicate::IsPresent, Some("*")),
        help_heading = "Arguments",
    )]
    targets: Vec<MaybeStdinVec<String>>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // convert targets to pkgs
        let mut iter = Targets::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .finalize_pkgs(self.targets.iter().flatten())?
            .ebuild_pkgs()
            .log_errors(self.ignore);

        let selected: IndexSet<_> = self.arches.iter().cloned().collect();
        let arch_filter = |arch: &Arch| -> bool { self.prefix || !arch.is_prefix() };

        let mut stdout = io::stdout().lock();
        for pkg in &mut iter {
            // determine default repo arches
            let mut enabled = pkg
                .repo()
                .arches()
                .iter()
                .filter(|&x| arch_filter(x))
                .cloned()
                .collect();
            // filter defaults by selected arches
            TriState::enabled(&mut enabled, &selected);

            // TODO: support tabular output formats
            let mut keywords = pkg.keywords().iter().filter(|x| enabled.contains(x.arch()));
            writeln!(stdout, "{pkg}: {}", keywords.join(" "))?;
        }

        Ok(ExitCode::from(iter))
    }
}
