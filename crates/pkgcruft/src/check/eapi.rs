use once_cell::sync::Lazy;
use pkgcraft::pkg::{ebuild::Pkg, Package};
use pkgcraft::repo::ebuild::Repo;

use crate::report::{
    Report,
    ReportKind::{EapiBanned, EapiDeprecated},
};

use super::{Check, CheckKind, CheckRun};

pub(super) static CHECK: Lazy<Check> =
    Lazy::new(|| Check::build(CheckKind::Eapi).reports([EapiBanned, EapiDeprecated]));

#[derive(Debug)]
pub(crate) struct EapiCheck<'a> {
    repo: &'a Repo,
}

impl<'a> EapiCheck<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self { repo }
    }
}

impl<'a> CheckRun<&Pkg<'a>> for EapiCheck<'a> {
    fn run(&self, pkg: &Pkg<'a>, reports: &mut Vec<Report>) {
        if self
            .repo
            .metadata()
            .config()
            .eapis_deprecated()
            .contains(pkg.eapi().as_ref())
        {
            reports.push(EapiDeprecated.version(pkg, pkg.eapi()));
        } else if self
            .repo
            .metadata()
            .config()
            .eapis_banned()
            .contains(pkg.eapi().as_ref())
        {
            reports.push(EapiBanned.version(pkg, pkg.eapi()));
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::TEST_DATA;

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let check_dir = repo.path().join(CHECK.as_ref());
        let scanner = Scanner::new().jobs(1).checks([&*CHECK]);
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
