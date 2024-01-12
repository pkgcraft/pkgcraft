use std::io::Write;

use strum::{AsRefStr, EnumIter, EnumString};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use crate::report::{Report, ReportLevel, ReportScope};
use crate::Error;

#[derive(AsRefStr, EnumIter, EnumString, Debug, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum Reporter {
    Simple(SimpleReporter),
    Fancy(FancyReporter),
    Json(JsonReporter),
}

impl Default for Reporter {
    fn default() -> Self {
        Reporter::Fancy(Default::default())
    }
}

impl Reporter {
    pub fn report(&mut self, report: &Report) -> crate::Result<()> {
        match self {
            Self::Simple(r) => r.report(report),
            Self::Fancy(r) => r.report(report),
            Self::Json(r) => r.report(report),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SimpleReporter {}

impl SimpleReporter {
    pub fn report(&mut self, report: &Report) -> crate::Result<()> {
        println!("{}: {}: {}", report.scope(), report.kind(), report.description());
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct FancyReporter {
    prev_key: Option<String>,
}

impl FancyReporter {
    pub fn report(&mut self, report: &Report) -> crate::Result<()> {
        let mut stdout = StandardStream::stdout(ColorChoice::Auto);

        let key = match report.scope() {
            ReportScope::Version(cpv) => cpv.cpn(),
            ReportScope::Package(cpn) => cpn.to_string(),
        };

        if key != self.prev_key.as_deref().unwrap_or_default() {
            if self.prev_key.is_some() {
                writeln!(&mut stdout)?;
            }
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue)).set_bold(true))?;
            writeln!(&mut stdout, "{key}")?;
            stdout.reset()?;
            self.prev_key = Some(key);
        }

        // determine report name color by its level
        let color = match report.level() {
            ReportLevel::Error => Color::Red,
            ReportLevel::Warning => Color::Yellow,
            ReportLevel::Style => Color::Cyan,
            ReportLevel::Info => Color::Green,
        };

        stdout.set_color(ColorSpec::new().set_fg(Some(color)))?;
        write!(&mut stdout, "  {}", report.kind())?;
        stdout.reset()?;

        write!(&mut stdout, ": ")?;
        if let ReportScope::Version(cpv) = report.scope() {
            write!(&mut stdout, "version {}: ", cpv.version())?;
        }
        writeln!(&mut stdout, "{}", report.description())?;

        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct JsonReporter {}

impl JsonReporter {
    pub fn report(&self, report: &Report) -> crate::Result<()> {
        let json = serde_json::to_string(&report)
            .map_err(|e| Error::InvalidValue(format!("failed serializing report to JSON: {e}")))?;
        println!("{}", json);
        Ok(())
    }
}
