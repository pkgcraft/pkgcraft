use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::pkg::ebuild::keyword::{cmp_arches, KeywordStatus::Unstable};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::ReportKind::UnstableOnly;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::CheckContext;

pub(super) static CHECK: super::Check = super::Check {
    name: "UnstableOnly",
    scope: Scope::Package,
    source: SourceKind::Ebuild,
    reports: &[UnstableOnly],
    context: &[CheckContext::Gentoo],
    priority: 0,
};

#[derive(Debug)]
pub(crate) struct Check<'a> {
    stable: HashSet<&'a str>,
}

impl<'a> Check<'a> {
    pub(crate) fn new(repo: &'a Repo) -> Self {
        Self {
            stable: repo
                .metadata
                .arches_desc()
                .get("stable")
                .map(|x| x.iter().map(|s| s.as_str()).collect())
                .unwrap_or_default(),
        }
    }
}

impl super::PackageCheckRun for Check<'_> {
    fn run(&self, pkgs: &[Pkg], filter: &mut ReportFilter) {
        let arches = pkgs
            .iter()
            .flat_map(|pkg| pkg.keywords())
            // select keywords allowed stable in the repo
            .filter(|kw| self.stable.contains(kw.arch()))
            .map(|kw| (kw.arch(), kw))
            // collapse keywords into an arch->keyword mapping
            .collect::<OrderedMap<_, OrderedSet<_>>>()
            .into_iter()
            // find arches that only have unstable keywords
            .filter(|(_, v)| v.iter().all(|k| k.status() == Unstable))
            .map(|(k, _)| k)
            .sorted_by(|a, b| cmp_arches(a, b))
            .collect::<Vec<_>>();

        if !arches.is_empty() {
            let message = arches.into_iter().join(", ");
            filter.report(UnstableOnly.package(pkgs, message));
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::TEST_DATA;
    use pretty_assertions::assert_eq;

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // gentoo unfixed
        let repo = TEST_DATA.repo("gentoo").unwrap();
        let check_dir = repo.path().join(&CHECK);
        let scanner = Scanner::new().jobs(1).checks([&CHECK]);
        let expected = glob_reports!("{check_dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // empty repo
        let repo = TEST_DATA.repo("empty").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }

    // TODO: scan with check selected vs unselected in non-gentoo repo once #194 is fixed
}
