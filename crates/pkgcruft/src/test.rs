use assert_cmd::Command;
use glob::glob;

use crate::report::{Iter, Report};

pub trait ToReports {
    fn to_reports(&mut self) -> Result<Vec<Report>, String>;
}

impl ToReports for Command {
    fn to_reports(&mut self) -> Result<Vec<Report>, String> {
        let output = self.output().unwrap();
        let data = String::from_utf8(output.stdout).unwrap();
        if output.status.success() {
            Ok(data
                .lines()
                .map(|s| Report::from_json(s).unwrap())
                .collect())
        } else {
            let err = String::from_utf8(output.stderr).unwrap();
            Err(format!("command failed: {err}"))
        }
    }
}

/// Return an iterator of reports from a globbed path pattern.
pub fn glob_reports_iter<P: AsRef<str>>(
    pattern: P,
) -> impl Iterator<Item = crate::Result<Report>> {
    glob(pattern.as_ref()).unwrap().flat_map(|path| {
        let path = path.unwrap();
        let path = path
            .to_str()
            .unwrap_or_else(|| panic!("invalid path: {path:?}"));
        Iter::try_from_file(path, None, None, None).unwrap()
    })
}

/// Return a vector of reports for the given globbed path patterns.
#[macro_export]
macro_rules! glob_reports {
    // handle comma-separated patterns with a trailing comma
    ($($pattern:expr,)+) => {{
        let mut reports = vec![];
        $(
            let glob = format!($pattern);
            let deserialized = $crate::test::glob_reports_iter(&glob)
                .collect::<$crate::Result<Vec<_>>>().unwrap();
            assert!(!deserialized.is_empty(), "no reports matching: {glob}");
            reports.extend(deserialized);
        )+
        reports
    }};

    // rewrite pattern args to use a trailing comma
    ($($pattern:expr),+) => {{
        glob_reports!($($pattern,)+)
    }};
}
pub use glob_reports;

/// Verify two, ordered report iterables are equal.
#[macro_export]
macro_rules! assert_ordered_reports {
    ($iter1:expr, $iter2:expr, $msg:expr) => {{
        let iter1 = $iter1
            .into_iter()
            .map(|x| $crate::report::Report::to_string(&x));
        let iter2 = $iter2
            .into_iter()
            .map(|x| $crate::report::Report::to_string(&x));
        pkgcraft::test::assert_ordered_eq!(iter1, iter2, $msg);
    }};

    ($iter1:expr, $iter2:expr $(,)?) => {{
        assert_ordered_reports!($iter1, $iter2, "");
    }};
}
pub use assert_ordered_reports;

/// Verify two, unordered report iterables contain the same content.
#[macro_export]
macro_rules! assert_unordered_reports {
    ($iter1:expr, $iter2:expr, $msg:expr) => {{
        let iter1 = $iter1
            .into_iter()
            .map(|x| $crate::report::Report::to_string(&x));
        let iter2 = $iter2
            .into_iter()
            .map(|x| $crate::report::Report::to_string(&x));
        pkgcraft::test::assert_unordered_eq!(iter1, iter2, $msg);
    }};

    ($iter1:expr, $iter2:expr $(,)?) => {{
        assert_unordered_reports!($iter1, $iter2, "");
    }};
}
pub use assert_unordered_reports;

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use super::*;

    #[test]
    fn to_reports() {
        // invalid option
        assert!(cmd("pkgcruft scan --invalid").to_reports().is_err());

        // empty repo
        let data = test_data();
        let repo = data.ebuild_repo("empty").unwrap();
        let reports = cmd("pkgcruft scan").arg(repo).to_reports().unwrap();
        assert!(reports.is_empty());
    }
}
