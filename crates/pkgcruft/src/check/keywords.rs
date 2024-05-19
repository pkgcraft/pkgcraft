use itertools::Itertools;
use pkgcraft::pkg::ebuild::keyword::Keyword;
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::{Report, ReportKind, VersionReport};
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{Check, CheckKind, CheckRun, EbuildPkgCheckKind};

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::EbuildPkg(EbuildPkgCheckKind::Keywords),
    source: SourceKind::Ebuild,
    scope: Scope::Package,
    priority: 0,
    reports: &[
        ReportKind::Version(VersionReport::OverlappingKeywords),
        ReportKind::Version(VersionReport::UnsortedKeywords),
    ],
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

        let overlapping: Vec<Vec<Keyword<_>>> = pkg
            .keywords()
            .iter()
            .map(|k| (k.arch(), k.status()))
            .collect::<OrderedMap<_, OrderedSet<_>>>()
            .into_iter()
            .filter(|(_, statuses)| statuses.len() > 1)
            .map(|(arch, statuses)| statuses.into_iter().map(|s| (s, arch).into()).collect())
            .collect();

        if !overlapping.is_empty() {
            let keywords = overlapping
                .iter()
                .map(|keywords| format!("({})", keywords.iter().sorted().join(", ")))
                .join(", ");
            reports.push(OverlappingKeywords.report(pkg, keywords));
        }

        let mut sorted_keywords = pkg.keywords().clone();
        sorted_keywords.sort();

        if &sorted_keywords != pkg.keywords() {
            let keywords = pkg.keywords().iter().join(" ");
            reports.push(UnsortedKeywords.report(pkg, keywords));
        }
    }
}
