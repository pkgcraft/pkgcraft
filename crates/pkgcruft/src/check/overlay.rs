use indexmap::IndexSet;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::{EbuildRepo, Eclass};

use crate::report::ReportKind::EclassUnused;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Overlay,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[EclassUnused],
    // drop optional when eclass deprecation flags are used instead
    context: &[CheckContext::Optional, CheckContext::Overlay],
    priority: 0,
};

pub(super) fn create(repo: &EbuildRepo) -> impl EbuildPkgCheck {
    let mut eclasses = repo.eclasses().clone();
    for repo in repo.masters() {
        for pkg in repo {
            for eclass in pkg.inherited() {
                eclasses.swap_remove(eclass);
            }
        }
    }

    Check { eclasses }
}

struct Check {
    eclasses: IndexSet<Eclass>,
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        for eclass in self.eclasses.intersection(pkg.inherited()) {
            EclassUnused.version(pkg).message(eclass).report(filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_unordered_eq, TEST_DATA};

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // secondary unfixed
        let repo = TEST_DATA.repo("qa-secondary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new().checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, expected);
    }
}
