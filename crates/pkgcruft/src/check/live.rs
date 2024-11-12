use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::Pkg;

use crate::report::ReportKind::LiveOnly;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, EbuildPkgSetCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Live,
    scope: Scope::Package,
    source: SourceKind::EbuildPkg,
    reports: &[LiveOnly],
    context: &[CheckContext::Gentoo],
    priority: 0,
};

pub(super) fn create() -> impl EbuildPkgSetCheck {
    Check
}

struct Check;

super::register!(Check);

impl EbuildPkgSetCheck for Check {
    fn run(&self, cpn: &Cpn, pkgs: &[Pkg], filter: &mut ReportFilter) {
        if pkgs.iter().all(|pkg| pkg.live()) {
            LiveOnly.package(cpn).report(filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_ordered_eq, TEST_DATA, TEST_DATA_PATCHED};

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // gentoo unfixed
        let repo = TEST_DATA.repo("gentoo").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new().jobs(1).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_ordered_eq!(reports, expected);

        // empty repo
        let repo = TEST_DATA.repo("empty").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_ordered_eq!(reports, []);

        // gentoo fixed
        let repo = TEST_DATA_PATCHED.repo("gentoo").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_ordered_eq!(reports, []);
    }

    // TODO: scan with check selected vs unselected in non-gentoo repo once #194 is fixed
}
