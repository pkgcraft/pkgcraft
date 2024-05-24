use pkgcraft::dep::parse::restrict_dependency;
use pkgcraft::dep::DependencySet;
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{
    Report,
    ReportKind::{self, MissingTestRestrict},
};

pub(super) static REPORTS: &[ReportKind] = &[MissingTestRestrict];

#[derive(Debug)]
pub(crate) struct Check<'a> {
    _repo: &'a Repo,
    restricts: DependencySet<String, String>,
}

impl<'a> Check<'a> {
    pub(super) fn new(_repo: &'a Repo) -> Self {
        Self {
            _repo,
            restricts: ["test", "!test? ( test )"]
                .iter()
                .map(|s| {
                    restrict_dependency(s).unwrap_or_else(|e| panic!("invalid RESTRICT: {s}: {e}"))
                })
                .collect(),
        }
    }
}

impl<'a> super::CheckRun<&Pkg<'a>> for Check<'a> {
    fn run<F: FnMut(Report)>(&self, pkg: &Pkg<'a>, mut report: F) {
        if pkg.iuse().contains("test")
            && pkg
                .restrict()
                .intersection(&self.restricts)
                .next()
                .is_none()
        {
            let message = r#"missing RESTRICT="!test? ( test )" with IUSE=test'"#;
            report(MissingTestRestrict.version(pkg, message));
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{TEST_DATA, TEST_DATA_PATCHED};
    use pretty_assertions::assert_eq;

    use crate::check::CheckKind::MissingTestRestrict;
    use crate::scanner::Scanner;
    use crate::test::*;

    #[test]
    fn check() {
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let check_dir = repo.path().join(MissingTestRestrict);
        let scanner = Scanner::new().jobs(1).checks([MissingTestRestrict]);
        let expected = glob_reports!("{check_dir}/*/reports.json");

        // check dir restriction
        let restrict = repo.restrict_from_path(&check_dir).unwrap();
        let reports: Vec<_> = scanner.run(repo, [&restrict]).collect();
        assert_eq!(&reports, &expected);

        // repo restriction
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);
    }

    #[test]
    fn patched() {
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let scanner = Scanner::new().jobs(1).checks([MissingTestRestrict]);
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}
