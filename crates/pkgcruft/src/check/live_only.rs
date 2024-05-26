use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::traits::Contains;

use crate::report::{
    Report,
    ReportKind::{self, LiveOnly},
};

pub(super) static REPORTS: &[ReportKind] = &[LiveOnly];

#[derive(Debug)]
pub(crate) struct Check;

impl super::CheckRun<&[Pkg<'_>]> for Check {
    fn run<F: FnMut(Report)>(&self, pkgs: &[Pkg<'_>], mut report: F) {
        if pkgs.iter().all(|pkg| pkg.properties().contains("live")) {
            report(LiveOnly.package(pkgs, "all versions are VCS-based"))
        }
    }
}
