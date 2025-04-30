use std::io::{self, Write};
use std::process::ExitCode;

use clap::{Args, builder::ArgPredicate};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::cli::{MaybeStdinVec, Targets, TriState};
use pkgcraft::config::Config;
use pkgcraft::pkg::ebuild::keyword::{Arch, KeywordStatus};
use pkgcraft::pkg::{Package, RepoPackage};
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
        // determine pkg targets
        let pkg_targets = Targets::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .finalize_pkgs(self.targets.iter().flatten())?;

        // determine default repo arches
        let selected: IndexSet<_> = self.arches.iter().cloned().collect();
        let mut arches: IndexSet<_> = pkg_targets
            .ebuild_repos()
            .flat_map(|repo| repo.arches())
            .filter(|arch| !arch.is_prefix() || self.prefix)
            .cloned()
            .collect();
        // filter defaults by selected arches
        TriState::enabled(&mut arches, selected);

        let mut b = Builder::new();
        if !arches.is_empty() {
            b.push_record(
                std::iter::once(String::new())
                    .chain(arches.iter().map(|a| a.to_string().chars().join("\n")))
                    .chain(["repo".chars().join("\n")]),
            );
        }

        // convert pkg targets to ebuild pkgs
        let mut iter = pkg_targets.ebuild_pkgs().log_errors(self.ignore);
        let mut stdout = io::stdout().lock();

        for pkg in &mut iter {
            let cpv = pkg.cpv().to_string();
            let repo = pkg.repo().to_string();
            let keywords = pkg
                .keywords()
                .iter()
                .filter(|x| arches.contains(x.arch()))
                .map(|k| {
                    match k.status() {
                        KeywordStatus::Disabled => "-",
                        KeywordStatus::Stable => "+",
                        KeywordStatus::Unstable => "~",
                    }
                    .to_string()
                })
                .pad_using(arches.len(), |_| " ".to_string());

            b.push_record(std::iter::once(cpv).chain(keywords).chain([repo]));
        }

        let mut table = b.build();
        if !table.is_empty() {
            table.with(Style::psql());
            writeln!(stdout, "{table}")?;
        }

        Ok(ExitCode::from(iter))
    }
}
