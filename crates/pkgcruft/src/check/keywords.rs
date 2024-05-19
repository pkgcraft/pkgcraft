use itertools::Itertools;
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{Report, ReportKind, VersionReport};
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{Check, CheckKind, CheckRun, EbuildPkgCheckKind};

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::EbuildPkg(EbuildPkgCheckKind::Keywords),
    source: SourceKind::Ebuild,
    scope: Scope::Package,
    priority: 0,
    reports: &[ReportKind::Version(VersionReport::UnsortedKeywords)],
};

#[derive(Debug)]
pub(crate) struct KeywordsCheck<'a> {
    _repo: &'a Repo,
}

impl<'a> KeywordsCheck<'a> {
    pub(super) fn new(_repo: &'a Repo) -> Self {
        Self { _repo }
    }
}

impl<'a> CheckRun<&Pkg<'a>> for KeywordsCheck<'a> {
    fn run(&self, pkg: &Pkg<'a>, reports: &mut Vec<Report>) {
        use VersionReport::*;

        let mut sorted_keywords = pkg.keywords().clone();
        sorted_keywords.sort();

        if &sorted_keywords != pkg.keywords() {
            let keywords = pkg.keywords().iter().join(" ");
            reports.push(UnsortedKeywords.report(pkg, keywords));
        }
    }
}
