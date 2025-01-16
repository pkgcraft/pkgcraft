use itertools::Itertools;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};

use crate::iter::ReportFilter;
use crate::report::{ReportKind::IgnoreUnused, ReportScope};

use super::{CpnCheck, CpvCheck, RepoCheck};

static CHECK: super::Check = super::Check::Ignore;

pub(super) struct Check;

super::register!(Check);

impl CpvCheck for Check {
    fn run(&self, _cpv: &Cpv, _filter: &ReportFilter) {}
    fn finish_target(&self, cpv: &Cpv, filter: &ReportFilter) {
        let scope = ReportScope::Version(cpv.clone(), None);

        // forciby populate the cache
        filter.ignore.generate(&scope).count();

        // flag unused version scope ignore directives
        if let Some(sets) = filter.ignore.unused(&scope) {
            let sets = sets.iter().join(", ");
            IgnoreUnused.version(cpv).message(sets).report(filter);
        }
    }
}

impl CpnCheck for Check {
    fn run(&self, _cpn: &Cpn, _filter: &ReportFilter) {}
    fn finish_target(&self, cpn: &Cpn, filter: &ReportFilter) {
        let scope = ReportScope::Package(cpn.clone());

        // forciby populate the cache
        filter.ignore.generate(&scope).count();

        // flag unused package scope ignore directives
        if let Some(sets) = filter.ignore.unused(&scope) {
            let sets = sets.iter().join(", ");
            IgnoreUnused.package(cpn).message(sets).report(filter);
        }
    }
}

impl RepoCheck for Check {
    fn run(&self, _repo: &EbuildRepo, _filter: &ReportFilter) {}
    fn finish_check(&self, repo: &EbuildRepo, filter: &ReportFilter) {
        let scope = ReportScope::Repo(repo.to_string());
        if let Some(sets) = filter.ignore.unused(&scope) {
            let sets = sets.iter().join(", ");
            IgnoreUnused.repo(repo).message(sets).report(filter);
        }

        for category in repo.categories() {
            let scope = ReportScope::Category(category.clone());
            if let Some(sets) = filter.ignore.unused(&scope) {
                let sets = sets.iter().join(", ");
                IgnoreUnused.category(category).message(sets).report(filter);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use crate::report::ReportSet;
    use crate::scan::Scanner;

    use super::*;

    #[test]
    fn check() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();

        // check isn't run by default
        let scanner = Scanner::new();
        let mut reports = scanner.run(repo, repo).unwrap();
        assert!(!reports.any(|r| CHECK.reports().contains(&r.kind)));

        // check run when all supported reports targeted
        let scanner = Scanner::new().reports([ReportSet::All]);
        let mut reports = scanner.run(repo, repo).unwrap();
        assert!(reports.any(|r| CHECK.reports().contains(&r.kind)));

        // verify reports in version scope
        let mut reports = scanner.run(repo, "Ignore/IgnoreUnused-0").unwrap();
        assert!(reports.any(|r| CHECK.reports().contains(&r.kind)));

        // verify reports in package scope
        let mut reports = scanner.run(repo, "Ignore/IgnoreUnused").unwrap();
        assert!(reports.any(|r| CHECK.reports().contains(&r.kind)));
    }
}
