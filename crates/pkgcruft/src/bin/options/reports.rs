use std::ops::Deref;

use clap::Args;
use indexmap::IndexSet;
use pkgcruft::report::{ReportKind, ReportTarget};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

#[derive(Debug, Args, Deserialize, Serialize, PartialEq, Eq)]
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
    pub reports: Vec<ReportTarget>,
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
        self.iter()
    }
}

impl Deref for Reports {
    type Target = [ReportTarget];

    fn deref(&self) -> &Self::Target {
        &self.reports
    }
}

#[cfg(test)]
mod test {
    use crate::options::reports::Reports;
    use pkgcruft::report::ReportTarget;

    #[test]
    fn serde() {
        let s = r#"["@all","-@<=warning"]"#;
        let reports: Vec<ReportTarget> = serde_json::from_str(s).unwrap();
        let r2 = Reports { reports };
        let json = serde_json::to_string(&r2).unwrap();

        let r: Reports = serde_json::from_str(&json).unwrap();

        assert_eq!(r, r2);
    }
}
