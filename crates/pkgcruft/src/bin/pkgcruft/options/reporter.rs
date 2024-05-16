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
            .map(|s| s.parse::<Reporter>().unwrap()),
    )]
    reporter: Reporter,

    /// Format string for the format reporter
    #[arg(long, required_if_eq("reporter", "format"))]
    format: Option<String>,
}

impl ReporterOptions {
    pub(crate) fn collapse(mut self) -> anyhow::Result<Reporter> {
        self.reporter.format(self.format)?;
        Ok(self.reporter)
    }
}
