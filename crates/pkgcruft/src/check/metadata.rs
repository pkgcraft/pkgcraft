use pkgcraft::dep::Cpv;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::restrict::Scope;
use pkgcraft::shell::pool::MetadataTaskBuilder;

use crate::iter::ReportFilter;
use crate::report::ReportKind::MetadataError;
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
    Check {
        regen: repo.pool().metadata_task(repo),
    }
}

struct Check {
    regen: MetadataTaskBuilder,
}

impl CpvCheck for Check {
    fn run(&self, cpv: &Cpv, filter: &mut ReportFilter) {
        match self.regen.run(cpv) {
            Err(InvalidPkg { err, .. }) => {
                MetadataError.version(cpv).message(err).report(filter)
            }
            Err(e) => unreachable!("{cpv}: unhandled metadata error: {e}"),
            Ok(_) => (),
        }
    }
}
