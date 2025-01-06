use indexmap::IndexSet;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::{EbuildRepo, Eclass};
use pkgcraft::restrict::Scope;

use crate::iter::ReportFilter;
use crate::report::ReportKind::EclassUnused;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Overlay,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[EclassUnused],
    // TODO: remove optional once eclass deprecation flags are used
    context: &[CheckContext::Overlay, CheckContext::Optional],
};

// TODO: use eclass deprecation flags instead
pub(super) fn create(repo: &EbuildRepo) -> impl EbuildPkgCheck {
    let mut eclasses = repo.eclasses().clone();
    for repo in repo.masters() {
        for pkg in repo.iter_unordered().filter_map(Result::ok) {
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
    fn run(&self, pkg: &EbuildPkg, filter: &ReportFilter) {
        for eclass in self.eclasses.intersection(pkg.inherited()) {
            EclassUnused.version(pkg).message(eclass).report(filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use crate::scan::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // secondary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-secondary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new(repo).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/optional.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);
    }
}
