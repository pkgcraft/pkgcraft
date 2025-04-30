use std::collections::HashMap;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::{Args, builder::ArgPredicate};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::cli::{MaybeStdinVec, Targets, TriState};
use pkgcraft::config::Config;
use pkgcraft::pkg::ebuild::keyword::{Arch, KeywordStatus};
use pkgcraft::pkg::{Package, RepoPackage};
use pkgcraft::repo::{PkgRepository, RepoFormat};
use pkgcraft::restrict::Scope;
use pkgcraft::traits::LogErrors;
use tabled::builder::Builder;
use tabled::settings::location::Locator;
use tabled::settings::object::{FirstRow, LastColumn};
use tabled::settings::style::HorizontalLine;
use tabled::settings::{Color, Style, Theme};

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
            .pkg_targets(self.targets.iter().flatten())?;

        let selected: IndexSet<_> = self.arches.iter().cloned().collect();
        let mut stdout = io::stdout().lock();
        let mut failed = false;

        let style = Style::modern()
            .remove_top()
            .remove_left()
            .remove_right()
            .remove_horizontal();
        let mut theme = Theme::from_style(style);
        let hline = HorizontalLine::inherit(Style::modern().remove_frame());
        theme.insert_horizontal_line(1, hline);

        // output a table per restriction target
        for (idx, (set, restrict)) in pkg_targets.iter().enumerate() {
            let scope = Scope::from(restrict);

            // determine default arch set
            let mut arches = IndexSet::new();
            let mut repos = 0;
            for repo in set.iter_ebuild() {
                arches.extend(
                    repo.arches()
                        .into_iter()
                        .filter(|arch| !arch.is_prefix() || self.prefix)
                        .cloned(),
                );
                repos += 1;
            }

            // filter defaults by selected arches
            TriState::enabled(&mut arches, selected.clone());

            // build table headers
            let mut builder = Builder::new();
            if !arches.is_empty() {
                let mut headers = vec![String::new()];
                headers.extend(arches.iter().map(|a| a.as_ref().chars().join("\n")));
                if repos > 1 {
                    headers.push("repo".chars().join("\n"));
                }
                builder.push_record(headers);
            }

            let mut iter = set.iter_restrict(restrict).log_errors(self.ignore);
            let mut target: Option<String> = None;
            for pkg in &mut iter {
                let pkg = pkg.into_ebuild().unwrap();

                // use versions for single package or version targets, otherwise use cpvs
                let mut row = vec![];
                if scope <= Scope::Package {
                    target.get_or_insert_with(|| pkg.cpn().to_string());
                    row.push(pkg.pvr());
                } else {
                    row.push(pkg.cpv().to_string());
                }

                let map: HashMap<_, _> = pkg
                    .keywords()
                    .iter()
                    .map(|k| (k.arch(), k.status()))
                    .collect();

                row.extend(arches.iter().map(|arch| {
                    match map.get(arch) {
                        Some(KeywordStatus::Disabled) => "-",
                        Some(KeywordStatus::Stable) => "+",
                        Some(KeywordStatus::Unstable) => "~",
                        None => " ",
                    }
                    .to_string()
                }));

                // only include repo data when multiple repos are targeted
                if repos > 1 {
                    row.push(pkg.repo().to_string());
                }

                builder.push_record(row);
            }
            failed |= iter.failed();

            // render table
            let mut table = builder.build();
            if !table.is_empty() {
                table.with(theme.clone());
                if repos > 1 {
                    table.modify(LastColumn, Color::FG_YELLOW);
                }
                table.modify(FirstRow, Color::FG_BRIGHT_WHITE);
                table.modify(Locator::content("+"), Color::FG_GREEN);
                table.modify(Locator::content("~"), Color::FG_BRIGHT_YELLOW);
                table.modify(Locator::content("-"), Color::FG_RED);

                // TODO: output raw targets for non-package scopes
                // output title for multiple package targets
                if pkg_targets.len() > 1 {
                    if let Some(target) = target {
                        if idx > 0 {
                            writeln!(stdout)?;
                        }
                        writeln!(stdout, "keywords for {target}:")?;
                    }
                }

                writeln!(stdout, "{table}")?;
            }
        }

        Ok(ExitCode::from(failed as u8))
    }
}
