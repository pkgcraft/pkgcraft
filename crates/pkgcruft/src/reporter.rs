use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use colored::{Color, Colorize};
use dashmap::DashMap;
use indexmap::IndexMap;
use itertools::Itertools;
use strfmt::strfmt;
use strum::{EnumString, VariantNames};

use crate::Error;
use crate::check::Check;
use crate::report::{Report, ReportKind, ReportScope};

#[derive(EnumString, VariantNames, Debug, Clone)]
#[strum(serialize_all = "kebab-case")]
pub enum Reporter {
    Count(CountReporter),
    Fancy(FancyReporter),
    Format(FormatReporter),
    Json(JsonReporter),
    Null,
    Simple(SimpleReporter),
    Stats(StatsReporter),
    Time(TimeReporter),
}

impl Reporter {
    /// Run a report through a reporter.
    pub fn report<W: Write>(&mut self, report: &Report, output: &mut W) -> crate::Result<()> {
        match self {
            Self::Count(r) => r.report(report, output),
            Self::Fancy(r) => r.report(report, output),
            Self::Format(r) => r.report(report, output),
            Self::Json(r) => r.report(report, output),
            Self::Null => Ok(()),
            Self::Simple(r) => r.report(report, output),
            Self::Stats(r) => r.report(report, output),
            Self::Time(_) => Ok(()),
        }
    }

    /// Perform any relevant reporter finalization.
    pub fn finish<W: Write>(&mut self, output: &mut W) -> crate::Result<()> {
        match self {
            Self::Count(r) => r.finish(output),
            Self::Stats(r) => r.finish(output),
            Self::Time(r) => r.finish(output),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct CountReporter(u64);

impl From<CountReporter> for Reporter {
    fn from(value: CountReporter) -> Self {
        Self::Count(value)
    }
}

impl CountReporter {
    fn report<W: Write>(&mut self, _report: &Report, _output: &mut W) -> crate::Result<()> {
        self.0 += 1;
        Ok(())
    }

    fn finish<W: Write>(&mut self, output: &mut W) -> crate::Result<()> {
        writeln!(output, "{}", self.0)?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct TimeReporter {
    pub stats: Arc<DashMap<Check, Duration>>,
}

impl From<TimeReporter> for Reporter {
    fn from(value: TimeReporter) -> Self {
        Self::Time(value)
    }
}

impl TimeReporter {
    fn finish<W: Write>(&mut self, output: &mut W) -> crate::Result<()> {
        for entry in self
            .stats
            .iter()
            .sorted_by(|e1, e2| e1.value().cmp(e2.value()))
        {
            let (check, time) = entry.pair();
            writeln!(output, "{check}: {time:.2?}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct StatsReporter {
    cache: IndexMap<ReportKind, u64>,
    pub sort_by: String,
}

impl From<StatsReporter> for Reporter {
    fn from(value: StatsReporter) -> Self {
        Self::Stats(value)
    }
}

impl StatsReporter {
    fn report<W: Write>(&mut self, report: &Report, _output: &mut W) -> crate::Result<()> {
        *self.cache.entry(report.kind).or_default() += 1;
        Ok(())
    }

    fn finish<W: Write>(&mut self, output: &mut W) -> crate::Result<()> {
        match self.sort_by.as_str() {
            "count" => self
                .cache
                .sort_by(|k1, v1, k2, v2| v1.cmp(v2).then_with(|| k1.cmp(k2))),
            "level" => self
                .cache
                .sort_by(|k1, _, k2, _| k1.level().cmp(&k2.level()).then_with(|| k1.cmp(k2))),
            _ => self.cache.sort_keys(),
        }

        for (kind, count) in &self.cache {
            write!(output, "{}", kind.as_ref().color(kind.level()))?;
            writeln!(output, ": {count}")?;
        }

        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct SimpleReporter;

impl From<SimpleReporter> for Reporter {
    fn from(value: SimpleReporter) -> Self {
        Self::Simple(value)
    }
}

impl SimpleReporter {
    fn report<W: Write>(&mut self, report: &Report, output: &mut W) -> crate::Result<()> {
        writeln!(output, "{report}")?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct FancyReporter {
    prev_key: Option<String>,
}

impl From<FancyReporter> for Reporter {
    fn from(value: FancyReporter) -> Self {
        Self::Fancy(value)
    }
}

impl FancyReporter {
    fn report<W: Write>(&mut self, report: &Report, output: &mut W) -> crate::Result<()> {
        let scope = report.scope();
        let key = if let ReportScope::Version(cpv, _) = scope {
            cpv.cpn().to_string()
        } else {
            scope.to_string()
        };

        if !self
            .prev_key
            .as_ref()
            .map(|prev| prev == &key)
            .unwrap_or_default()
        {
            if self.prev_key.is_some() {
                writeln!(output)?;
            }
            writeln!(output, "{}", key.color(Color::Blue).bold())?;
            self.prev_key = Some(key);
        }

        write!(output, "  {}", report.kind.as_ref().color(report.level()))?;

        if let ReportScope::Version(cpv, location) = scope {
            write!(output, ": version {}", cpv.version())?;
            if let Some(value) = location {
                write!(output, ", {value}")?;
            }
        }

        if let Some(value) = report.message() {
            write!(output, ": {value}")?;
        }

        writeln!(output)?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct JsonReporter;

impl From<JsonReporter> for Reporter {
    fn from(value: JsonReporter) -> Self {
        Self::Json(value)
    }
}

impl JsonReporter {
    fn report<W: Write>(&self, report: &Report, output: &mut W) -> crate::Result<()> {
        writeln!(output, "{}", report.to_json())?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct FormatReporter {
    pub format: String,
}

impl From<FormatReporter> for Reporter {
    fn from(value: FormatReporter) -> Self {
        Self::Format(value)
    }
}

impl FormatReporter {
    fn report<W: Write>(&self, report: &Report, output: &mut W) -> crate::Result<()> {
        let mut attrs: HashMap<_, _> = [("name".to_string(), report.kind.to_string())]
            .into_iter()
            .collect();

        match report.scope() {
            ReportScope::Version(cpv, _) => {
                let category = cpv.category().to_string();
                let package = cpv.package().to_string();
                let version = cpv.version().to_string();
                let ebuild = format!("{package}-{version}.ebuild");
                attrs.extend([
                    ("path".to_string(), format!("{category}/{package}/{ebuild}")),
                    ("ebuild".to_string(), ebuild),
                    ("category".to_string(), category),
                    ("package".to_string(), package),
                    ("version".to_string(), version),
                    ("cpv".to_string(), cpv.to_string()),
                    ("cpn".to_string(), cpv.cpn().to_string()),
                ]);
            }
            ReportScope::Package(cpn) => {
                attrs.extend([
                    ("category".to_string(), cpn.category().to_string()),
                    ("package".to_string(), cpn.package().to_string()),
                    ("cpn".to_string(), cpn.to_string()),
                ]);
            }
            ReportScope::Category(cat) => {
                attrs.extend([("category".to_string(), cat.to_string())]);
            }
            ReportScope::Repo(repo) => {
                attrs.extend([("repo".to_string(), repo.to_string())]);
            }
        }

        let s = strfmt(&self.format, &attrs).map_err(|e| {
            let supported = attrs.keys().sorted().join(", ");
            Error::InvalidValue(format!(
                "{}: invalid output format: {e}\n  [possible attributes: {supported}]",
                report.kind
            ))
        })?;
        if !s.is_empty() {
            writeln!(output, "{s}")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    static REPORTS: &str = indoc::indoc! {r#"
        {"scope":{"Version":["cat/pkg-1-r2",null]},"kind":"DependencyDeprecated","message":"BDEPEND: cat/deprecated"}
        {"scope":{"Version":["cat/pkg-1-r2",{"line":3,"column":0}]},"kind":"WhitespaceUnneeded","message":"empty line"}
        {"scope":{"Version":["cat/pkg-1-r2",{"line":3,"column":28}]},"kind":"WhitespaceInvalid","message":"character '\\u{2001}'"}
        {"scope":{"Package":"cat/pkg"},"kind":"UnstableOnly","message":"arch"}
        {"scope":{"Category":"cat1"},"kind":"RepoCategoryEmpty","message":null}
        {"scope":{"Category":"cat2"},"kind":"RepoCategoryEmpty","message":null}
        {"scope":{"Repo":"repo1"},"kind":"LicensesUnused","message":"unused"}
    "#};

    fn report<R: Into<Reporter>>(reporter: R) -> String {
        let mut reporter = reporter.into();
        let reports = REPORTS.lines().map(|x| Report::from_json(x).unwrap());
        let mut output = Vec::new();

        for report in reports {
            reporter.report(&report, &mut output).unwrap();
        }
        reporter.finish(&mut output).unwrap();

        String::from_utf8(output).unwrap()
    }

    #[test]
    fn count() {
        let output = report(CountReporter::default());
        assert_eq!("7", output.trim());
    }

    #[test]
    fn simple() {
        let expected = indoc::indoc! {r#"
            cat/pkg-1-r2: DependencyDeprecated: BDEPEND: cat/deprecated
            cat/pkg-1-r2, line 3: WhitespaceUnneeded: empty line
            cat/pkg-1-r2, line 3, column 28: WhitespaceInvalid: character '\u{2001}'
            cat/pkg: UnstableOnly: arch
            cat1/*: RepoCategoryEmpty
            cat2/*: RepoCategoryEmpty
            repo1: LicensesUnused: unused
        "#};

        let output = report(SimpleReporter);
        assert_eq!(expected, &output);
    }

    #[test]
    fn stats() {
        // sort by name
        let expected = indoc::indoc! {r#"
            DependencyDeprecated: 1
            LicensesUnused: 1
            RepoCategoryEmpty: 2
            UnstableOnly: 1
            WhitespaceInvalid: 1
            WhitespaceUnneeded: 1
        "#};
        let mut reporter = StatsReporter::default();
        let output = report(reporter.clone());
        assert_eq!(expected, &output);

        // sort by count
        let expected = indoc::indoc! {r#"
            DependencyDeprecated: 1
            LicensesUnused: 1
            UnstableOnly: 1
            WhitespaceInvalid: 1
            WhitespaceUnneeded: 1
            RepoCategoryEmpty: 2
        "#};
        reporter.sort_by = "count".to_string();
        let output = report(reporter.clone());
        assert_eq!(expected, &output);

        // sort by level
        let expected = indoc::indoc! {r#"
            DependencyDeprecated: 1
            LicensesUnused: 1
            RepoCategoryEmpty: 2
            WhitespaceInvalid: 1
            WhitespaceUnneeded: 1
            UnstableOnly: 1
        "#};
        reporter.sort_by = "level".to_string();
        let output = report(reporter.clone());
        assert_eq!(expected, &output);
    }

    #[test]
    fn fancy() {
        let expected = indoc::indoc! {r#"
            cat/pkg
              DependencyDeprecated: version 1-r2: BDEPEND: cat/deprecated
              WhitespaceUnneeded: version 1-r2, line 3: empty line
              WhitespaceInvalid: version 1-r2, line 3, column 28: character '\u{2001}'
              UnstableOnly: arch

            cat1/*
              RepoCategoryEmpty

            cat2/*
              RepoCategoryEmpty

            repo1
              LicensesUnused: unused
        "#};

        let output = report(FancyReporter::default());
        assert_eq!(expected, &output);
    }

    #[test]
    fn null() {
        let output = report(Reporter::Null);
        assert!(output.is_empty());
    }

    #[test]
    fn json() {
        let output = report(JsonReporter);
        assert_eq!(REPORTS, &output);
    }

    #[test]
    fn format() {
        let mut format_reporter = FormatReporter::default();

        // empty format string
        let output = report(format_reporter.clone());
        assert_eq!("", &output);

        // existing format strings
        let expected = indoc::indoc! {"
            DependencyDeprecated
            WhitespaceUnneeded
            WhitespaceInvalid
            UnstableOnly
            RepoCategoryEmpty
            RepoCategoryEmpty
            LicensesUnused
        "};
        format_reporter.format = "{name}".to_string();
        let output = report(format_reporter.clone());
        assert_eq!(expected, &output);
    }
}
