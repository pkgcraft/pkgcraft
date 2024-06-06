use itertools::Itertools;
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::traits::Contains;

use crate::report::ReportKind::{LiveKeywords, LiveOnly};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, PackageCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Live,
    scope: Scope::Package,
    source: SourceKind::Ebuild,
    reports: &[LiveKeywords, LiveOnly],
    context: &[CheckContext::Gentoo],
    priority: 0,
};

pub(super) fn create() -> impl PackageCheck {
    Check
}

struct Check;

super::register!(Check);

impl PackageCheck for Check {
    fn run(&self, pkgs: &[Pkg], filter: &mut ReportFilter) {
        let mut live_only = true;

        for pkg in pkgs {
            if !pkg.properties().contains("live") {
                live_only = false;
            } else if !pkg.keywords().is_empty() {
                let message = pkg.keywords().iter().join(", ");
                filter.report(LiveKeywords.version(pkg, message))
            }
        }

        if live_only {
            filter.report(LiveOnly.package(pkgs, ""))
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{TEST_DATA, TEST_DATA_PATCHED};
    use pretty_assertions::assert_eq;

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // gentoo unfixed
        let repo = TEST_DATA.repo("gentoo").unwrap();
        let check_dir = repo.path().join(CHECK);
        let scanner = Scanner::new().jobs(1).checks([CHECK]);
        let expected = glob_reports!("{check_dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // empty repo
        let repo = TEST_DATA.repo("empty").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);

        // gentoo fixed
        let repo = TEST_DATA_PATCHED.repo("gentoo").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }

    // TODO: scan with check selected vs unselected in non-gentoo repo once #194 is fixed
}
