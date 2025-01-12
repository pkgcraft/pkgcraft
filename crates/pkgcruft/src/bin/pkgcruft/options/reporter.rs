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

    /// Sorting variant for the stats reporter
    #[arg(
        long,
        default_value = "name",
        hide_possible_values = true,
        value_parser = ["name", "count", "level"],
    )]
    stats: Option<String>,
}

impl ReporterOptions {
    pub(crate) fn collapse(&self) -> Reporter {
        let mut reporter = self.reporter.clone();

        if let Reporter::Format(r) = &mut reporter {
            r.format = self.format.clone().unwrap_or_default();
        }

        if let Reporter::Stats(r) = &mut reporter {
            r.sort_by = self.stats.clone().unwrap_or_default();
        }

        reporter
    }
}
