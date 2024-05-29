use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::traits::Contains;

use crate::report::{
    Report,
    ReportKind::{self, LiveOnly},
};

pub(super) static REPORTS: &[ReportKind] = &[LiveOnly];

#[derive(Debug)]
pub(crate) struct Check;

impl super::CheckRun<&[Pkg<'_>]> for Check {
    fn run<F: FnMut(Report)>(&self, pkgs: &[Pkg<'_>], mut report: F) {
        if pkgs.iter().all(|pkg| pkg.properties().contains("live")) {
            report(LiveOnly.package(pkgs, "all versions are VCS-based"))
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{TEST_DATA, TEST_DATA_PATCHED};
    use pretty_assertions::assert_eq;

    use crate::check::CheckKind::LiveOnly;
    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    #[test]
    fn check() {
        // gentoo unfixed
        let repo = TEST_DATA.repo("gentoo").unwrap();
        let scanner = Scanner::new().jobs(1).checks([LiveOnly]);
        let check_dir = repo.path().join(LiveOnly);
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
