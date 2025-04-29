use std::io::{self, Write};
use std::process::ExitCode;

use clap::{Args, builder::ArgPredicate};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::cli::{MaybeStdinVec, Targets, TriState};
use pkgcraft::config::Config;
use pkgcraft::pkg::RepoPackage;
use pkgcraft::pkg::ebuild::keyword::{Arch, KeywordStatus};
use pkgcraft::repo::RepoFormat;
use pkgcraft::traits::LogErrors;
use tabled::builder::Builder;
use tabled::settings::Style;

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
        let pkg_targets = Targets::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .finalize_pkgs(self.targets.iter().flatten())?;

        let selected: IndexSet<_> = self.arches.iter().cloned().collect();
        let arch_filter = |arch: &Arch| -> bool { self.prefix || !arch.is_prefix() };

        let arches: IndexSet<_> = pkg_targets
            .ebuild_repo_restricts()
            .flat_map(|(repos, _)| repos.arches())
            .filter(|&x| arch_filter(x))
            .cloned()
            .collect();

        let mut b = Builder::new();
        if !arches.is_empty() {
            b.push_record(
                std::iter::once(String::new())
                    .chain(arches.iter().map(|a| a.to_string().chars().join("\n"))),
            );
        }

        let mut iter = pkg_targets.ebuild_pkgs().log_errors(self.ignore);

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
            b.push_record(
                std::iter::once(pkg.to_string()).chain(
                    pkg.keywords()
                        .iter()
                        .filter(|x| enabled.contains(x.arch()))
                        .map(|k| {
                            match k.status() {
                                KeywordStatus::Disabled => " ",
                                KeywordStatus::Stable => "+",
                                KeywordStatus::Unstable => "~",
                            }
                            .to_string()
                        }),
                ),
            );
        }

        let mut table = b.build();
        if !table.is_empty() {
            table.with(Style::psql());
            writeln!(stdout, "{table}")?;
        }

        Ok(ExitCode::from(iter))
    }
}
