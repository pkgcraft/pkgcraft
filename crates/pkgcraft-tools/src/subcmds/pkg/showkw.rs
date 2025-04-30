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

        let selected: IndexSet<_> = self.arches.iter().cloned().collect();
        let mut repos = IndexSet::new();
        let mut arches = IndexSet::new();

        // determine default arch set
        for repo in pkg_targets.ebuild_repos() {
            repos.insert(repo);
            arches.extend(
                repo.arches()
                    .into_iter()
                    .filter(|arch| !arch.is_prefix() || self.prefix)
                    .cloned(),
            );
        }

        // filter defaults by selected arches
        TriState::enabled(&mut arches, selected);
        let repos = repos.len();

        // build table headers
        let mut b = Builder::new();
        if !arches.is_empty() {
            let mut headers = vec![String::new()];
            headers.extend(arches.iter().map(|a| a.to_string().chars().join("\n")));
            if repos > 1 {
                headers.push("repo".chars().join("\n"));
            }
            b.push_record(headers);
        }

        // convert pkg targets to ebuild pkgs
        let mut iter = pkg_targets.ebuild_pkgs().log_errors(self.ignore);
        let mut stdout = io::stdout().lock();

        // insert table records
        for pkg in &mut iter {
            let mut row = vec![pkg.cpv().to_string()];
            row.extend(
                pkg.keywords()
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
                    .pad_using(arches.len(), |_| " ".to_string()),
            );
            if repos > 1 {
                row.push(pkg.repo().to_string());
            }
            b.push_record(row);
        }

        // render table
        let mut table = b.build();
        if !table.is_empty() {
            table.with(Style::psql());
            writeln!(stdout, "{table}")?;
        }

        Ok(ExitCode::from(iter))
    }
}
