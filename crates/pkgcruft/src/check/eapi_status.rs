use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::report::ReportKind::{EapiBanned, EapiDeprecated};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::{EbuildRawPkg, SourceKind};

use super::{CheckKind, EbuildRawPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::EapiStatus,
    scope: Scope::Version,
    source: SourceKind::EbuildRawPkg,
    reports: &[EapiBanned, EapiDeprecated],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildRawPkgCheck {
    Check { repo }
}

struct Check {
    repo: &'static EbuildRepo,
}

impl EbuildRawPkgCheck for Check {
    fn run(&self, pkg: &EbuildRawPkg, filter: &mut ReportFilter) {
        let eapi = pkg.eapi().as_str();
        if self.repo.metadata().config.eapis_deprecated.contains(eapi) {
            EapiDeprecated.version(pkg).message(eapi).report(filter);
        } else if self.repo.metadata().config.eapis_banned.contains(eapi) {
            EapiBanned.version(pkg).message(eapi).report(filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_unordered_eq, test_data, test_data_patched};

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // secondary with no banned or deprecated EAPIs set
        let repo = data.ebuild_repo("qa-secondary").unwrap();
        assert!(repo.path().join(CHECK).exists());
        let scanner = Scanner::new(repo).checks([CHECK]);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let scanner = Scanner::new(repo).checks([CHECK]);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
