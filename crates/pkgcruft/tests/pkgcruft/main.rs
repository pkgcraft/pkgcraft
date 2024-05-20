use assert_cmd::Command;
use pkgcruft::report::Report;

mod diff;
mod replay;
mod scan;
mod show;

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
