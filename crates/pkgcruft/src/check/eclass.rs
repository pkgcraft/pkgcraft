use dashmap::DashSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::{EbuildRepo, Eclass};
use pkgcraft::restrict::Scope;

use crate::iter::ReportFilter;
use crate::report::ReportKind::EclassUnused;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Eclass,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[EclassUnused],
    context: &[],
};

pub(super) fn create(repo: &EbuildRepo) -> impl EbuildPkgCheck {
    Check {
        unused: repo.metadata().eclasses().iter().cloned().collect(),
    }
}

struct Check {
    unused: DashSet<Eclass>,
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, _filter: &ReportFilter) {
        for eclass in pkg.inherited() {
            self.unused.remove(eclass);
        }
    }

    fn finish(&self, repo: &EbuildRepo, filter: &ReportFilter) {
        if filter.enabled(EclassUnused) && !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            EclassUnused.repo(repo).message(unused).report(filter);
        }
    }
}
