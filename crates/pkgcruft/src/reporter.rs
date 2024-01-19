use std::collections::HashMap;
use std::io::Write;

use colored::{Color, Colorize};
use strfmt::strfmt;
use strum::{AsRefStr, EnumIter, EnumString, EnumVariantNames};

use crate::report::{Report, ReportScope};
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
    pub fn format(&mut self, format: Option<String>) -> crate::Result<()> {
        match (self, format) {
            (Self::Format(r), format) => {
                if let Some(s) = format {
                    r.format = s;
                } else {
                    return Err(Error::InvalidValue(
                        "format reporter requires a format string".to_string(),
                    ));
                }
            }
            (_, Some(_)) => {
                return Err(Error::InvalidValue(
                    "format option only valid with the format reporter".to_string(),
                ))
            }
            _ => (),
        }

        Ok(())
    }

    /// Run a report through a reporter.
    pub fn report(&mut self, report: &Report, output: &mut dyn Write) -> crate::Result<()> {
        match self {
            Self::Simple(r) => r.report(report, output),
            Self::Fancy(r) => r.report(report, output),
            Self::Json(r) => r.report(report, output),
            Self::Format(r) => r.report(report, output),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SimpleReporter {}

impl SimpleReporter {
    fn report(&mut self, report: &Report, output: &mut dyn Write) -> crate::Result<()> {
        writeln!(output, "{}: {}: {}", report.scope(), report.kind(), report.description())?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct FancyReporter {
    prev_key: Option<String>,
}

impl FancyReporter {
    fn report(&mut self, report: &Report, output: &mut dyn Write) -> crate::Result<()> {
        let key = match report.scope() {
            ReportScope::Version(cpv) => cpv.cpn(),
            ReportScope::Package(cpn) => cpn.to_string(),
        };

        if key != self.prev_key.as_deref().unwrap_or_default() {
            if self.prev_key.is_some() {
                writeln!(output)?;
            }
            writeln!(output, "{}", key.color(Color::Blue).bold())?;
            self.prev_key = Some(key);
        }

        write!(output, "  {}", report.kind().as_ref().color(report.level()))?;

        write!(output, ": ")?;
        if let ReportScope::Version(cpv) = report.scope() {
            write!(output, "version {}: ", cpv.version())?;
        }
        writeln!(output, "{}", report.description())?;

        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct JsonReporter {}

impl JsonReporter {
    fn report(&self, report: &Report, output: &mut dyn Write) -> crate::Result<()> {
        writeln!(output, "{}", report.to_json()?)?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct FormatReporter {
    format: String,
}

impl FormatReporter {
    fn report(&self, report: &Report, output: &mut dyn Write) -> crate::Result<()> {
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
            writeln!(output, "{s}")?;
        }

        Ok(())
    }
}
