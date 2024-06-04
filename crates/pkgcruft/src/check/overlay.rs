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

impl VersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        for eclass in self.eclasses.intersection(pkg.inherited()) {
            filter.report(EclassUnused.version(pkg, eclass))
        }
    }
}
