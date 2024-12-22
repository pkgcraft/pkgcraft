use pkgcraft::dep::Cpv;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::report::ReportKind::MetadataError;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, CpvCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Metadata,
    scope: Scope::Version,
    source: SourceKind::Cpv,
    reports: &[MetadataError],
    context: &[],
};

pub(super) fn create(repo: &EbuildRepo) -> impl CpvCheck {
    Check { repo: repo.clone() }
}

struct Check {
    repo: EbuildRepo,
}

impl CpvCheck for Check {
    fn run(&self, cpv: &Cpv, filter: &mut ReportFilter) {
        if let Err(e) = self.repo.generate_pkg_metadata(cpv, false, false) {
            match e {
                InvalidPkg { err, .. } => MetadataError.version(cpv).message(err).report(filter),
                _ => unreachable!("{cpv}: unhandled metadata error: {e}"),
            }
        }
    }
}
