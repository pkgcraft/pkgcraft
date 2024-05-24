use assert_cmd::Command;
use glob::glob;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::test::TEST_DATA;

use crate::report::{Iter, Report};

/// Return the ebuild repo object for a given shared test data repo.
pub fn qa_repo(name: &str) -> &Repo {
    TEST_DATA.ebuild_repo(name).unwrap()
}

pub trait ToReports {
    fn to_reports(&mut self) -> Vec<Report>;
}

impl ToReports for Command {
    fn to_reports(&mut self) -> Vec<Report> {
        let output = self.output().unwrap().stdout;
        let data = String::from_utf8(output).unwrap();
        data.lines()
            .map(|s| Report::from_json(s).unwrap())
            .collect()
    }
}

/// Return an iterator of reports from a globbed path pattern.
pub fn glob_reports_iter<P: AsRef<str>>(pattern: P) -> impl Iterator<Item = Report> {
    glob(pattern.as_ref())
        .unwrap()
        .filter_map(Result::ok)
        .flat_map(|path| {
            let path = path
                .to_str()
                .unwrap_or_else(|| panic!("invalid path: {path:?}"));
            Iter::try_from_file(path, None, None)
                .unwrap()
                .filter_map(Result::ok)
        })
}

/// Return a vector of reports for the given globbed path patterns.
#[macro_export]
macro_rules! glob_reports {
    // handle comma-separated patterns with a trailing comma
    ($($pattern:expr,)+) => {{
        let mut reports = vec![];
        $(reports.extend($crate::test::glob_reports_iter(format!($pattern)));)+
        reports
    }};

    // rewrite pattern args to use a trailing comma
    ($($pattern:expr),+) => {{
        glob_reports!($($pattern,)+)
    }};
}
pub use glob_reports;
