use std::ops::Deref;

use clap::Args;
use indexmap::IndexSet;
use pkgcruft::report::{ReportKind, Reports};
use strum::IntoEnumIterator;

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Report options"))]
pub(crate) struct ReportOptions {
    /// Restrict by report target
    #[arg(
        short,
        long,
        value_name = "TARGET[,...]",
        value_delimiter = ',',
        allow_hyphen_values = true
    )]
    reports: Vec<Reports>,
}

impl ReportOptions {
    /// Return the set of report variants enabled for replaying.
    pub(crate) fn replay(&self) -> pkgcruft::Result<IndexSet<ReportKind>> {
        let defaults: IndexSet<_> = ReportKind::iter().collect();
        let (enabled, _) = Reports::collapse(self, &defaults, &defaults)?;
        Ok(enabled)
    }
}

impl<'a> IntoIterator for &'a ReportOptions {
    type Item = &'a Reports;
    type IntoIter = std::slice::Iter<'a, Reports>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl Deref for ReportOptions {
    type Target = [Reports];

    fn deref(&self) -> &Self::Target {
        &self.reports
    }
}
