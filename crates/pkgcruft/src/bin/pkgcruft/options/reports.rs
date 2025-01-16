use std::ops::Deref;

use clap::Args;
use indexmap::IndexSet;
use pkgcruft::report::{ReportKind, ReportTarget};
use strum::IntoEnumIterator;

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Report options"))]
pub(crate) struct Reports {
    /// Restrict by report target
    #[arg(
        short,
        long,
        value_name = "TARGET[,...]",
        value_delimiter = ',',
        allow_hyphen_values = true
    )]
    reports: Vec<ReportTarget>,
}

impl Reports {
    /// Return the set of report variants enabled for replaying.
    pub(crate) fn replay(&self) -> pkgcruft::Result<IndexSet<ReportKind>> {
        let defaults: IndexSet<_> = ReportKind::iter().collect();
        let (enabled, _) = ReportTarget::collapse(self, &defaults, &defaults)?;
        Ok(enabled)
    }
}

impl<'a> IntoIterator for &'a Reports {
    type Item = &'a ReportTarget;
    type IntoIter = std::slice::Iter<'a, ReportTarget>;

    fn into_iter(self) -> Self::IntoIter {
        self.reports.iter()
    }
}

impl Deref for Reports {
    type Target = [ReportTarget];

    fn deref(&self) -> &Self::Target {
        &self.reports
    }
}
