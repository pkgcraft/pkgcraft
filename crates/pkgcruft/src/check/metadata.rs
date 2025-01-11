use pkgcraft::dep::Cpv;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::shell::pool::MetadataTaskBuilder;

use crate::iter::ReportFilter;
use crate::report::ReportKind::MetadataError;

use super::CpvCheck;

pub(super) fn create(repo: &EbuildRepo) -> impl CpvCheck {
    Check {
        regen: repo.pool().metadata_task(repo),
    }
}

static CHECK: super::Check = super::Check::Metadata;

struct Check {
    regen: MetadataTaskBuilder,
}

super::register!(Check);

impl CpvCheck for Check {
    fn run(&self, cpv: &Cpv, filter: &ReportFilter) {
        if let Err(InvalidPkg { err, .. }) = self.regen.run(cpv) {
            MetadataError.version(cpv).message(err).report(filter)
        }
    }
}
