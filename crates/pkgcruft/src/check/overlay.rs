use indexmap::IndexSet;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::{EbuildRepo, Eclass};
use pkgcraft::repo::PkgRepository;

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
    context: &[CheckContext::Overlay],
    priority: 0,
};

// TODO: use eclass deprecation flags instead
pub(super) fn create(repo: &EbuildRepo) -> impl EbuildPkgCheck {
    let mut eclasses = repo.eclasses().clone();
    for repo in repo.masters() {
        for pkg in repo.iter().filter_map(Result::ok) {
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
    use pkgcraft::test::{assert_unordered_eq, test_data};

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // secondary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-secondary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);
    }
}
