use std::collections::HashMap;
use std::io::Write;
use std::process::ExitCode;

use clap::Args;
use clap::builder::{ArgPredicate, PossibleValuesParser, TypedValueParser};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::cli::{MaybeStdinVec, Targets, TriState};
use pkgcraft::config::Config;
use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::pkg::ebuild::keyword::{Arch, KeywordStatus};
use pkgcraft::pkg::{Package, RepoPackage};
use pkgcraft::repo::set::RepoSet;
use pkgcraft::repo::{PkgRepository, RepoFormat};
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
#[derive(Debug, Clone)]
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
        let style = match self {
            Self::Ascii => Style::ascii(),
            Self::Modern => Style::modern(),
        };

        // remove default separators
        let style = style
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

    /// Apply a theme to a table format.
    fn style(&self, table: &mut Table, mut theme: TableTheme) {
        let repo_col = table.count_columns() - 1;
        theme.insert_vline(2);
        theme.insert_vline(repo_col);
        theme.insert_vline(repo_col - 2);
        table.with(theme.inner);
        table.with(Alignment::bottom());
        table.modify(Columns::first(), Padding::zero());
        table.modify(Columns::one(1), Padding::new(0, 1, 0, 0));
        table.modify(Columns::new(2..repo_col - 3), Padding::new(1, 0, 0, 0));
        table.modify(Columns::new(repo_col - 2..repo_col - 1), Padding::new(1, 0, 0, 0));
    }
}

/// Determine target architectures to output.
fn determine_arches(
    set: &RepoSet,
    selected: &IndexSet<TriState<Arch>>,
    prefix_enabled: bool,
) -> anyhow::Result<IndexSet<Arch>> {
    // determine default arch set
    let all_arches: IndexSet<_> = set
        .iter_ebuild()
        .flat_map(|r| r.arches())
        .cloned()
        .collect();
    let mut target_arches: IndexSet<_> = all_arches
        .iter()
        .filter(|arch| !arch.is_prefix() || prefix_enabled)
        .cloned()
        .collect();

    // determine target arches, filtering defaults by selected arches
    TriState::enabled(&mut target_arches, selected.clone());

    // verify target arches exist
    let mut nonexistent = target_arches.difference(&all_arches).peekable();
    if nonexistent.peek().is_some() {
        let nonexistent = nonexistent.join(", ");
        anyhow::bail!("nonexistent arches: {nonexistent}");
    }

    Ok(target_arches)
}

/// Iterator collecting packages into Cpn groups.
struct CpnPkgsIter<'a, I: Iterator<Item = EbuildPkg>> {
    iter: &'a mut I,
    prev_cpn: Option<Cpn>,
    pkgs: Vec<EbuildPkg>,
}

impl<'a, I: Iterator<Item = EbuildPkg>> CpnPkgsIter<'a, I> {
    fn new(iter: &'a mut I) -> Self {
        Self {
            iter,
            prev_cpn: Default::default(),
            pkgs: Default::default(),
        }
    }
}

impl<I: Iterator<Item = EbuildPkg>> Iterator for CpnPkgsIter<'_, I> {
    type Item = (Cpn, Vec<EbuildPkg>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(pkg) = self.iter.next() {
                let prev_cpn = self.prev_cpn.get_or_insert_with(|| pkg.cpn().clone());
                if prev_cpn != pkg.cpn() {
                    let cpn = self.prev_cpn.replace(pkg.cpn().clone()).unwrap();
                    let pkgs = std::mem::take(&mut self.pkgs);
                    self.pkgs.push(pkg);
                    return Some((cpn, pkgs));
                } else {
                    self.pkgs.push(pkg);
                }
            } else if let Some(cpn) = self.prev_cpn.take() {
                let pkgs = std::mem::take(&mut self.pkgs);
                return Some((cpn, pkgs));
            } else {
                return None;
            }
        }
    }
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // determine pkg targets
        let pkg_targets = Targets::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .pkg_targets(self.targets.iter().flatten())?;

        let selected_arches = self.arches.iter().cloned().collect();
        let mut stdout = anstream::stdout().lock();
        let mut failed = false;
        let mut idx = 0;

        // output a table per restriction target
        for (set, restrict) in pkg_targets {
            let mut theme = self.format.theme();
            let target_arches = determine_arches(&set, &selected_arches, self.prefix)?;

            // build table headers
            let mut builder = Builder::new();
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

            // determine ebuild pkgs from target restriction
            let mut iter = set
                .iter_restrict(restrict)
                .map(|result| result.and_then(EbuildPkg::try_from))
                .log_errors(self.ignore);
            let cpn_pkgs_iter = CpnPkgsIter::new(&mut iter);

            for (cpn, pkgs) in cpn_pkgs_iter {
                let mut prev_slot = None;
                let mut pkg_row = 0;
                let mut builder = builder.clone();
                idx += 1;

                for pkg in pkgs {
                    pkg_row += 1;
                    let mut row = vec![];

                    // determine pkg status
                    let mut statuses = PkgStatus::from_pkg(&pkg).peekable();
                    if statuses.peek().is_some() {
                        row.push(format!("[{}]", statuses.join("")));
                    } else {
                        row.push("".to_string());
                    }

                    row.push(pkg.pvr());

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

                    let slot = pkg.fullslot();
                    if !prev_slot
                        .as_ref()
                        .map(|prev| prev == slot)
                        .unwrap_or_default()
                    {
                        theme.insert_hline(pkg_row);
                        row.push(slot.to_string());
                        prev_slot = Some(slot.clone());
                    } else {
                        row.push("".to_string());
                    }

                    row.push(Color::FG_YELLOW.colorize(pkg.repo()));

                    builder.push_record(row);
                }

                // render table
                let mut table = builder.build();
                // apply table formatting
                self.format.style(&mut table, theme.clone());
                // force vertical header output
                table.modify(Rows::first(), Width::wrap(1));

                // add blank line between tables
                if idx > 1 {
                    writeln!(stdout)?;
                }

                writeln!(stdout, "keywords for {cpn}:")?;

                // strip trailing whitespace from rendered table lines
                for line in table.to_string().lines() {
                    writeln!(stdout, "{}", line.trim_end())?;
                }
            }

            // combine all failure statuses
            failed |= iter.failed();
        }

        Ok(ExitCode::from(failed as u8))
    }
}
