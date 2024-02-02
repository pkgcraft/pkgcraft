use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::pkg::ebuild::keyword::{cmp_arches, KeywordStatus::Unstable};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::{PackageReport, Report, ReportKind};
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{Check, CheckKind, CheckRun, EbuildPkgSetCheckKind};

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::EbuildPkgSet(EbuildPkgSetCheckKind::UnstableOnly),
    source: SourceKind::EbuildPackage,
    scope: Scope::Package,
    priority: 0,
    reports: &[ReportKind::Package(PackageReport::UnstableOnly)],
};

#[derive(Debug)]
pub(crate) struct UnstableOnlyCheck<'a> {
    arches: HashSet<&'a str>,
}

impl<'a> UnstableOnlyCheck<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        let arches = if let Some(arches) = repo.metadata().arches_desc().get("stable") {
            arches.iter().map(|s| s.as_str()).collect()
        } else {
            Default::default()
        };
        Self { arches }
    }
}

impl<'a> CheckRun<&[Pkg<'a>]> for UnstableOnlyCheck<'a> {
    fn run(&self, pkgs: &[Pkg<'a>], reports: &mut Vec<Report>) {
        use PackageReport::*;

        let arches: Vec<_> = pkgs
            .iter()
            .flat_map(|pkg| pkg.keywords())
            // select keywords allowed stable in the repo
            .filter(|kw| self.arches.contains(kw.arch()))
            .map(|kw| (kw.arch(), kw))
            // collapse keywords into an arch->keyword mapping
            .collect::<OrderedMap<_, OrderedSet<_>>>()
            .into_iter()
            // find arches that only have unstable keywords
            .filter(|(_, v)| v.iter().all(|k| k.status() == Unstable))
            .map(|(k, _)| k)
            .collect();

        if !arches.is_empty() {
            let arches = arches.iter().sorted_by(|a, b| cmp_arches(a, b)).join(", ");
            reports.push(UnstableOnly.report(pkgs, arches));
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::TEST_DATA;

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
        let check_dir = repo.path().join(CHECK.kind().as_ref());
        let restrict = repo.restrict_from_path(&check_dir).unwrap();
        let scanner = Scanner::new().jobs(1).checks(&[CHECK.kind()]);
        let expected: Vec<_> = glob_reports(format!("{check_dir}/*/reports.json")).collect();
        let reports: Vec<_> = scanner.run(repo, [&restrict]).collect();
        assert_eq!(&reports, &expected);
    }
}
