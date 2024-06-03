use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::traits::Contains;

use crate::report::ReportKind::LiveOnly;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::CheckContext;

pub(super) static CHECK: super::Check = super::Check {
    name: "LiveOnly",
    scope: Scope::Package,
    source: SourceKind::Ebuild,
    reports: &[LiveOnly],
    context: &[CheckContext::Gentoo],
    priority: 0,
    create,
};

fn create(_repo: &Repo) -> super::Runner {
    super::Runner::LiveOnly(Check)
}

#[derive(Debug)]
pub(crate) struct Check;

impl super::CheckRun<&[Pkg<'_>]> for Check {
    fn run(&self, pkgs: &[Pkg<'_>], filter: &mut ReportFilter) {
        if pkgs.iter().all(|pkg| pkg.properties().contains("live")) {
            filter.report(LiveOnly.package(pkgs, "all versions are VCS-based"))
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
        let check_dir = repo.path().join(&CHECK);
        let scanner = Scanner::new().jobs(1).checks([&CHECK]);
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
