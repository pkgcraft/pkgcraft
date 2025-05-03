use std::collections::HashMap;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use clap::builder::{ArgPredicate, PossibleValuesParser, TypedValueParser};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::cli::{MaybeStdinVec, Targets, TriState};
use pkgcraft::config::Config;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::pkg::ebuild::keyword::{Arch, KeywordStatus};
use pkgcraft::pkg::{Package, RepoPackage};
use pkgcraft::repo::{PkgRepository, RepoFormat};
use pkgcraft::restrict::Scope;
use pkgcraft::traits::LogErrors;
use strum::{Display, EnumIter, EnumString, IntoEnumIterator, VariantNames};
use tabled::settings::object::{Columns, Rows};
use tabled::settings::style::{HorizontalLine, VerticalLine};
use tabled::settings::{Alignment, Color, Padding, Style, Theme, Width};
use tabled::{Table, builder::Builder};

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

    /// Set the tabular format
    #[arg(
        short,
        long,
        default_value = "modern",
        hide_default_value = true,
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(TableFormat::VARIANTS)
            .map(|s| s.parse::<TableFormat>().unwrap()),
    )]
    format: TableFormat,

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

/// Package status variants.
#[derive(EnumIter, Debug, PartialEq, Eq, Hash, Copy, Clone)]
enum PkgStatus {
    Deprecated,
    Masked,
}

impl PkgStatus {
    /// Return the iterator of statuses from a package.
    fn from_pkg(pkg: &EbuildPkg) -> impl Iterator<Item = Self> {
        Self::iter().filter(|status| match status {
            Self::Deprecated => pkg.deprecated(),
            Self::Masked => pkg.masked(),
        })
    }
}

impl std::fmt::Display for PkgStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deprecated => write!(f, "{}", Color::FG_YELLOW.colorize("D")),
            Self::Masked => write!(f, "{}", Color::FG_RED.colorize("M")),
        }
    }
}

/// Formatting theme variants for tabular output.
#[derive(Display, EnumIter, EnumString, VariantNames, Debug, PartialEq, Eq, Copy, Clone)]
#[strum(serialize_all = "kebab-case")]
enum TableFormat {
    Ascii,
    Modern,
}

/// Wrapper for tabular table theming.
struct TableTheme {
    inner: Theme,
    format: TableFormat,
}

impl TableTheme {
    /// Insert a vertical divider in a theme at a column location.
    fn insert_vline(&mut self, column: usize) {
        let vline = match self.format {
            TableFormat::Ascii => VerticalLine::inherit(Style::ascii().remove_frame()),
            TableFormat::Modern => VerticalLine::inherit(Style::modern().remove_frame()),
        };
        self.inner.insert_vertical_line(column, vline);
    }

    /// Insert a horizontal divider in a theme at a row location.
    fn insert_hline(&mut self, row: usize) {
        let hline = match self.format {
            TableFormat::Ascii => HorizontalLine::inherit(Style::ascii().remove_frame()),
            TableFormat::Modern => HorizontalLine::inherit(Style::modern().remove_frame()),
        };
        self.inner.insert_horizontal_line(row, hline);
    }
}

impl TableFormat {
    /// Create a theme for a table format.
    fn theme(self) -> TableTheme {
        match self {
            Self::Ascii => {
                let style = Style::blank()
                    .remove_vertical()
                    .horizontal('-')
                    .remove_horizontal();
                TableTheme {
                    inner: Theme::from_style(style),
                    format: self,
                }
            }
            Self::Modern => {
                let style = Style::modern()
                    .remove_top()
                    .remove_left()
                    .remove_right()
                    .remove_bottom()
                    .remove_vertical()
                    .remove_horizontal();
                TableTheme {
                    inner: Theme::from_style(style),
                    format: self,
                }
            }
        }
    }

    /// Apply a theme to a table format.
    fn style(&self, table: &mut Table, mut theme: TableTheme) {
        let repo_col = table.count_columns() - 1;
        theme.insert_vline(2);
        theme.insert_vline(repo_col);
        theme.insert_vline(repo_col - 2);
        table.with(theme.inner);
        table.with(Alignment::bottom());
        table.modify(Columns::first(), Padding::zero());
        table.modify(Columns::single(1), Padding::new(0, 1, 0, 0));
        table.modify(Columns::new(2..repo_col - 3), Padding::new(1, 0, 0, 0));
        table.modify(Columns::new(repo_col - 2..repo_col - 1), Padding::new(1, 0, 0, 0));
    }
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // determine pkg targets
        let pkg_targets = Targets::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .pkg_targets(self.targets.iter().flatten())?;

        let selected_arches: IndexSet<_> = self.arches.iter().cloned().collect();
        let mut stdout = io::stdout().lock();
        let mut failed = false;

        // output a table per restriction target
        for (idx, (set, restrict)) in pkg_targets.iter().enumerate() {
            let mut theme = self.format.theme();
            let scope = Scope::from(restrict);

            // determine default arch set
            let all_arches: IndexSet<_> = set
                .iter_ebuild()
                .flat_map(|r| r.arches())
                .cloned()
                .collect();
            let mut target_arches: IndexSet<_> = all_arches
                .iter()
                .filter(|arch| !arch.is_prefix() || self.prefix)
                .cloned()
                .collect();

            // determine target arches, filtering defaults by selected arches
            TriState::enabled(&mut target_arches, selected_arches.clone());

            // verify target arches exist
            let nonexistent: Vec<_> = target_arches.difference(&all_arches).collect();
            if !nonexistent.is_empty() {
                let nonexistent = nonexistent.iter().join(", ");
                anyhow::bail!("nonexistent arches: {nonexistent}");
            }

            // build table headers
            let mut builder = Builder::new();
            if !target_arches.is_empty() {
                let mut headers = vec![String::new(), String::new()];
                headers.extend(
                    target_arches
                        .iter()
                        .map(|a| Color::FG_BRIGHT_WHITE.colorize(a)),
                );
                headers.push("eapi".to_string());
                headers.push("slot".to_string());
                headers.push("repo".to_string());
                builder.push_record(headers);
            }

            // determine ebuild pkgs from target restriction
            let mut iter = set
                .iter_restrict(restrict)
                .map(|result| result.and_then(EbuildPkg::try_from))
                .log_errors(self.ignore);

            let mut target: Option<String> = None;
            let mut prev_slot = None;
            let mut pkg_row = 0;
            for pkg in &mut iter {
                pkg_row += 1;
                let mut row = vec![];

                // determine pkg status
                let statuses: Vec<_> = PkgStatus::from_pkg(&pkg).collect();
                if !statuses.is_empty() {
                    row.push(format!("[{}]", statuses.iter().join("")));
                } else {
                    row.push("".to_string());
                }

                // Vary pkg identifier used by target scope.
                //
                // Versions for single package or version targets, otherwise cpvs.
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

                row.extend(target_arches.iter().map(|arch| match map.get(arch) {
                    Some(KeywordStatus::Disabled) => Color::FG_RED.colorize("-"),
                    Some(KeywordStatus::Stable) => Color::FG_GREEN.colorize("+"),
                    Some(KeywordStatus::Unstable) => Color::FG_BRIGHT_YELLOW.colorize("~"),
                    None => " ".to_string(),
                }));

                row.push(Color::FG_BRIGHT_GREEN.colorize(pkg.eapi()));

                let slot = pkg.slot().to_string();
                if !prev_slot
                    .as_ref()
                    .map(|prev| prev == &slot)
                    .unwrap_or_default()
                {
                    theme.insert_hline(pkg_row);
                    row.push(slot.clone());
                    prev_slot = Some(slot);
                } else {
                    row.push("".to_string());
                }

                row.push(Color::FG_YELLOW.colorize(pkg.repo()));

                builder.push_record(row);
            }
            failed |= iter.failed();

            // render table
            let mut table = builder.build();
            if !table.is_empty() {
                // apply table formatting
                self.format.style(&mut table, theme);
                // force vertical header output
                table.modify(Rows::first(), Width::wrap(1));

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
