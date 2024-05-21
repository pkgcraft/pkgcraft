use assert_cmd::Command;
use camino::Utf8Path;
use pkgcraft::repo::Repository;
use pkgcraft::test::TEST_DATA;
use pkgcruft::report::Report;

mod diff;
mod replay;
mod scan;
mod show;

/// Return the path to a given QA repo.
pub(crate) fn qa_repo(name: &str) -> &Utf8Path {
    TEST_DATA.ebuild_repo(name).unwrap().path()
}
pub(crate) trait ToReports {
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
