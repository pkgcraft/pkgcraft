use pkgcraft::pkg::{ebuild::Pkg, Package};
use pkgcraft::repo::ebuild::Repo;

use crate::report::{
    Report,
    ReportKind::{self, EapiBanned, EapiDeprecated},
};

pub(super) static REPORTS: &[ReportKind] = &[EapiBanned, EapiDeprecated];

#[derive(Debug)]
pub(crate) struct Check<'a> {
    repo: &'a Repo,
}

impl<'a> Check<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self { repo }
    }
}

impl<'a> super::CheckRun<&Pkg<'a>> for Check<'a> {
    fn run<F: FnMut(Report)>(&self, pkg: &Pkg<'a>, mut report: F) {
        let eapi = pkg.eapi().as_ref();
        if self.repo.metadata.config.eapis_deprecated.contains(eapi) {
            report(EapiDeprecated.version(pkg, eapi));
        } else if self.repo.metadata.config.eapis_banned.contains(eapi) {
            report(EapiBanned.version(pkg, eapi));
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::TEST_DATA;
    use pretty_assertions::assert_eq;

    use crate::check::CheckKind::Eapi;
    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    #[test]
    fn check() {
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let check_dir = repo.path().join(Eapi.as_ref());
        let scanner = Scanner::new().jobs(1).checks([Eapi]);
        let expected = glob_reports!("{check_dir}/*/reports.json");

        // check dir restriction
        let restrict = repo.restrict_from_path(&check_dir).unwrap();
        let reports: Vec<_> = scanner.run(repo, [&restrict]).collect();
        assert_eq!(&reports, &expected);

        // repo restriction
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);
    }
}
