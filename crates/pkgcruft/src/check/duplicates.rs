use pkgcraft::dep::Cpn;
use pkgcraft::repo::Repository;
use pkgcraft::restrict::Scope;
use pkgcraft::traits::Contains;

use crate::report::ReportKind::PackageOverride;
use crate::scan::ScannerRun;
use crate::source::SourceKind;

use super::Context::{Optional, Overlay};

super::register! {
    super::Check {
        kind: super::CheckKind::Duplicates,
        reports: &[PackageOverride],
        scope: Scope::Package,
        sources: &[SourceKind::Cpn],
        context: &[Optional, Overlay],
        create,
    }
}

pub(super) fn create(_run: &ScannerRun) -> super::Runner {
    Box::new(Check)
}

struct Check;

impl super::CheckRun for Check {
    fn run_cpn(&self, cpn: &Cpn, run: &ScannerRun) {
        for repo in run.repo.masters() {
            if repo.contains(cpn) {
                PackageOverride
                    .package(cpn)
                    .message(format!("repo: {}", repo.name()))
                    .report(run);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::{assert_err_re, test_data};

    use crate::scan::Scanner;

    use super::*;

    #[test]
    fn check() {
        // optional check isn't run by default
        let data = test_data();
        let repo = data.ebuild_repo("qa-secondary").unwrap();
        let scanner = Scanner::new();
        let mut reports = scanner.run(repo, repo).unwrap();
        assert!(!reports.any(|r| CHECK.reports.contains(&r.kind)));

        let scanner = Scanner::new().reports([CHECK]);

        // primary
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let r = scanner.run(repo, repo);
        assert_err_re!(r, "Duplicates: check requires overlay context");

        // secondary
        let repo = data.ebuild_repo("qa-secondary").unwrap();
        let mut reports = scanner.run(repo, repo).unwrap();
        assert!(reports.any(|r| CHECK.reports.contains(&r.kind)));
    }
}
