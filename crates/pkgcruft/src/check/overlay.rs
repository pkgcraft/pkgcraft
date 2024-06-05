use indexmap::IndexSet;
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::{Eclass, Repo};

use crate::report::ReportKind::EclassUnused;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, VersionCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Overlay,
    scope: Scope::Version,
    source: SourceKind::Ebuild,
    reports: &[EclassUnused],
    context: &[CheckContext::Overlay],
    priority: 0,
};

pub(super) fn create(repo: &'static Repo) -> impl VersionCheck {
    let mut eclasses: IndexSet<_> = repo.eclasses().values().collect();
    for repo in repo.masters() {
        for pkg in &*repo {
            for eclass in pkg.inherited() {
                eclasses.swap_remove(*eclass);
            }
        }
    }

    Check { eclasses }
}

struct Check {
    eclasses: IndexSet<&'static Eclass>,
}

super::register!(Check);

impl VersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        for eclass in self.eclasses.intersection(pkg.inherited()) {
            filter.report(EclassUnused.version(pkg, eclass))
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::TEST_DATA;
    use pretty_assertions::assert_eq;

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // secondary unfixed
        let repo = TEST_DATA.repo("qa-secondary").unwrap();
        let check_dir = repo.path().join(CHECK);
        let scanner = Scanner::new().jobs(1).checks([CHECK]);
        let expected = glob_reports!("{check_dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);
    }
}
