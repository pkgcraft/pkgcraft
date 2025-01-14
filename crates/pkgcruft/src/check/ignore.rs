use itertools::Itertools;
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::iter::ReportFilter;
use crate::report::ReportKind::IgnoreUnused;

use super::RepoCheck;

pub(super) fn create() -> impl RepoCheck {
    Check
}

static CHECK: super::Check = super::Check::Ignore;

struct Check;

super::register!(Check);

impl RepoCheck for Check {
    fn run(&self, _repo: &EbuildRepo, _filter: &ReportFilter) {}
    fn finish(&self, repo: &EbuildRepo, filter: &ReportFilter) {
        if filter.enabled(IgnoreUnused) {
            for (path, sets) in filter.ignore.unused() {
                let sets = sets.iter().join(", ");
                IgnoreUnused
                    .repo(repo)
                    .message(format!("{path}: {sets}"))
                    .report(filter);
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
        // requires running in non-filtered context
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let scanner = Scanner::new(repo).reports([CHECK]);
        let r = scanner.run(repo);
        assert_err_re!(r, "Ignore: check requires unfiltered context");

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
