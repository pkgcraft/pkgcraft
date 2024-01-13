use std::str::FromStr;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Args;
use pkgcruft::reporter::Reporter;
use strum::VariantNames;

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Reporter options"))]
pub(crate) struct ReporterOptions {
    /// Reporter to use
    #[arg(
        short = 'R',
        long,
        default_value = "fancy",
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(Reporter::VARIANTS)
            .map(|s| Reporter::from_str(&s).unwrap()),
    )]
    reporter: Reporter,

    /// Template for the template reporter
    #[arg(short, long)]
    template: Option<String>,
}

impl ReporterOptions {
    pub(crate) fn collapse(mut self) -> anyhow::Result<Reporter> {
        self.reporter.template(self.template.unwrap_or_default())?;
        Ok(self.reporter)
    }
}
