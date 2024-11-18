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
    priority: -9999,
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl CpvCheck {
    Check { repo }
}

struct Check {
    repo: &'static EbuildRepo,
}

super::register!(Check);

impl CpvCheck for Check {
    fn run(&self, cpv: &Cpv, filter: &mut ReportFilter) {
        match self.repo.update_pkg_metadata(cpv.clone(), false, false) {
            Ok(_) => (),
            Err(InvalidPkg { id: _, err }) => {
                MetadataError.version(cpv).message(err).report(filter)
            }
            Err(e) => panic!("{cpv}: unhandled metadata error: {e}"),
        }
    }
}
