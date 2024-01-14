use std::collections::HashMap;
use std::io::Write;

use strfmt::strfmt;
use strum::{AsRefStr, EnumIter, EnumString, EnumVariantNames};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use crate::report::{Report, ReportLevel, ReportScope};
use crate::Error;

#[derive(AsRefStr, EnumIter, EnumString, EnumVariantNames, Debug, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum Reporter {
    Simple(SimpleReporter),
    Fancy(FancyReporter),
    Json(JsonReporter),
    Format(FormatReporter),
}

impl Default for Reporter {
    fn default() -> Self {
        Reporter::Fancy(Default::default())
    }
}

impl Reporter {
    /// Inject a format string into compatible reporter variants.
    pub fn format(&mut self, format: String) -> crate::Result<()> {
        match (self, format) {
            (Self::Format(r), s) if !s.is_empty() => r.format = s,
            (Self::Format(_), _) => {
                return Err(Error::InvalidValue(
                    "format reporter requires non-empty format".to_string(),
                ))
            }
            (_, s) if !s.is_empty() => {
                return Err(Error::InvalidValue(
                    "format only valid with format reporter".to_string(),
                ))
            }
            _ => (),
        }
        Ok(())
    }

    /// Run a report through a reporter.
    pub fn report(&mut self, report: &Report) -> crate::Result<()> {
        match self {
            Self::Simple(r) => r.report(report),
            Self::Fancy(r) => r.report(report),
            Self::Json(r) => r.report(report),
            Self::Format(r) => r.report(report),
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
        println!("{}", report.to_json()?);
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct FormatReporter {
    format: String,
}

impl FormatReporter {
    pub fn report(&self, report: &Report) -> crate::Result<()> {
        let mut attrs: HashMap<_, _> = [("name".to_string(), report.kind().to_string())]
            .into_iter()
            .collect();

        match report.scope() {
            ReportScope::Version(cpv) => {
                attrs.extend([
                    ("category".to_string(), cpv.category().to_string()),
                    ("package".to_string(), cpv.package().to_string()),
                    ("version".to_string(), cpv.version().to_string()),
                ]);
            }
            ReportScope::Package(cpn) => attrs.extend([
                ("category".to_string(), cpn.category().to_string()),
                ("package".to_string(), cpn.package().to_string()),
            ]),
        }

        let s = strfmt(&self.format, &attrs)
            .map_err(|e| Error::InvalidValue(format!("formatting failed: {e}")))?;
        if !s.is_empty() {
            println!("{s}");
        }

        Ok(())
    }
}
