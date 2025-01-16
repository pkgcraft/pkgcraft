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
    /// Return an iterator over the selected report targets.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &ReportTarget> {
        self.reports.iter()
    }

    /// Return true if no reports are selected.
    pub(crate) fn is_empty(&self) -> bool {
        self.reports.is_empty()
    }

    /// Return the set of report variants enabled for replaying.
    pub(crate) fn replay(&self) -> pkgcruft::Result<IndexSet<ReportKind>> {
        let defaults: IndexSet<_> = ReportKind::iter().collect();
        let (enabled, _) = ReportTarget::collapse(&self.reports, &defaults, &defaults)?;
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
