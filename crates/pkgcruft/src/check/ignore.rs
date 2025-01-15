use itertools::Itertools;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::iter::ReportFilter;
use crate::report::{ReportKind::IgnoreUnused, ReportScope};

use super::{CpnCheck, CpvCheck, RepoCheck};

static CHECK: super::Check = super::Check::Ignore;

pub(super) struct Check;

super::register!(Check);

impl CpvCheck for Check {
    fn run(&self, _cpv: &Cpv, _filter: &ReportFilter) {}
    fn finish(&self, _repo: &EbuildRepo, filter: &ReportFilter) {
        if filter.enabled(IgnoreUnused) {
            for (scope, sets) in filter.ignore.unused() {
                if let ReportScope::Version(cpv, _) = scope {
                    let sets = sets.iter().join(", ");
                    IgnoreUnused.version(cpv).message(sets).report(filter);
                }
            }
        }
    }
}

impl CpnCheck for Check {
    fn run(&self, _cpn: &Cpn, _filter: &ReportFilter) {}
    fn finish(&self, _repo: &EbuildRepo, filter: &ReportFilter) {
        if filter.enabled(IgnoreUnused) {
            for (scope, sets) in filter.ignore.unused() {
                if let ReportScope::Package(cpn) = scope {
                    let sets = sets.iter().join(", ");
                    IgnoreUnused.package(cpn).message(sets).report(filter);
                }
            }
        }
    }
}

impl RepoCheck for Check {
    fn run(&self, _repo: &EbuildRepo, _filter: &ReportFilter) {}
    fn finish(&self, repo: &EbuildRepo, filter: &ReportFilter) {
        if filter.enabled(IgnoreUnused) {
            for (scope, sets) in filter.ignore.unused() {
                let sets = sets.iter().join(", ");
                if let ReportScope::Repo(_) = scope {
                    IgnoreUnused.repo(repo).message(sets).report(filter);
                } else if let ReportScope::Category(category) = scope {
                    IgnoreUnused.category(category).message(sets).report(filter);
                }
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
        let scanner = Scanner::new(repo);
        let mut reports = scanner.run(repo).unwrap();
        assert!(!reports.any(|r| CHECK.reports().contains(&r.kind)));

        // check run when all supported reports targeted
        let scanner = Scanner::new(repo).reports([ReportSet::All]);
        let mut reports = scanner.run(repo).unwrap();
        assert!(reports.any(|r| CHECK.reports().contains(&r.kind)));
    }
}
