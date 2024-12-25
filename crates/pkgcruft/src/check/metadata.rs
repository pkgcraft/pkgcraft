use std::sync::Arc;

use pkgcraft::dep::Cpv;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::restrict::Scope;
use pkgcraft::shell::BuildPool;

use crate::report::ReportKind::MetadataError;
use crate::scanner::ReportFilter;
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
        repo: repo.clone(),
        pool: repo.pool(),
    }
}

struct Check {
    repo: EbuildRepo,
    pool: Arc<BuildPool>,
}

impl CpvCheck for Check {
    fn run(&self, cpv: &Cpv, filter: &mut ReportFilter) {
        match self.pool.metadata(&self.repo, cpv, false, false) {
            Err(InvalidPkg { err, .. }) => {
                MetadataError.version(cpv).message(err).report(filter)
            }
            #[cfg_attr(coverage, coverage(off))]
            Err(e) => unreachable!("{cpv}: unhandled metadata error: {e}"),
            Ok(_) => (),
        }
    }
}
