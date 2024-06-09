use pkgcraft::pkg::{ebuild::Pkg, Package};
use pkgcraft::repo::ebuild::Repo;

use crate::report::ReportKind::{EapiBanned, EapiDeprecated};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, VersionCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::EapiStatus,
    scope: Scope::Version,
    source: SourceKind::Ebuild,
    reports: &[EapiBanned, EapiDeprecated],
    context: &[],
    priority: 0,
    parse: false,
};

pub(super) fn create(repo: &'static Repo) -> impl VersionCheck {
    Check { repo }
}

struct Check {
    repo: &'static Repo,
}

super::register!(Check);

impl VersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        let eapi = pkg.eapi().as_ref();
        if self.repo.metadata.config.eapis_deprecated.contains(eapi) {
            filter.report(EapiDeprecated.version(pkg, eapi));
        } else if self.repo.metadata.config.eapis_banned.contains(eapi) {
            filter.report(EapiBanned.version(pkg, eapi));
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
        // primary unfixed
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new().jobs(1).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // secondary with no banned or deprecated EAPIs set
        let repo = TEST_DATA.repo("qa-secondary").unwrap();
        assert!(repo.path().join(CHECK).exists());
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}
