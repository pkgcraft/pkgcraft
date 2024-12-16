use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::report::ReportKind::ManifestInvalid;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, EbuildPkgSetCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Manifest,
    scope: Scope::Package,
    source: SourceKind::EbuildPkg,
    reports: &[ManifestInvalid],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildPkgSetCheck {
    Check { repo }
}

struct Check {
    repo: &'static EbuildRepo,
}

impl EbuildPkgSetCheck for Check {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildPkg], filter: &mut ReportFilter) {
        let manifest = match self.repo.metadata().pkg_manifest_parse(cpn) {
            Ok(value) => value,
            Err(e) => {
                ManifestInvalid.package(cpn).message(e).report(filter);
                return;
            }
        };

        let manifest_distfiles: IndexSet<_> = manifest.distfiles().map(|x| x.name()).collect();
        let pkg_distfiles: IndexSet<_> = pkgs.iter().flat_map(|p| p.distfiles()).collect();

        let unknown = manifest_distfiles
            .difference(&pkg_distfiles)
            .sorted()
            .join(", ");
        if !unknown.is_empty() {
            ManifestInvalid
                .package(cpn)
                .message(format!("unknown: {unknown}"))
                .report(filter);
        }

        let missing = pkg_distfiles
            .difference(&manifest_distfiles)
            .sorted()
            .join(", ");
        if !missing.is_empty() {
            ManifestInvalid
                .package(cpn)
                .message(format!("missing: {missing}"))
                .report(filter);
        }
    }
}
