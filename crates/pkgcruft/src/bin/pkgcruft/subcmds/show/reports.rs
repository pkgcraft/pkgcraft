use std::collections::HashSet;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Args;
use colored::Colorize;
use strum::{IntoEnumIterator, VariantNames};

use pkgcruft::report::{ReportLevel, REPORTS};

#[derive(Debug, Args)]
pub struct Subcommand {
    /// Output specific levels
    #[arg(
        short,
        long,
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(ReportLevel::VARIANTS)
            .map(|s| s.parse::<ReportLevel>().unwrap()),
    )]
    levels: Vec<ReportLevel>,
}

impl Subcommand {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let levels: HashSet<_> = if self.levels.is_empty() {
            ReportLevel::iter().collect()
        } else {
            self.levels.into_iter().collect()
        };

        let mut stdout = io::stdout().lock();
        for report in &*REPORTS {
            if levels.contains(&report.level()) {
                writeln!(stdout, "{}", report.as_ref().color(report.level()))?;
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
