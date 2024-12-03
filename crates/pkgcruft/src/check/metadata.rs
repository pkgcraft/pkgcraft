use std::sync::Arc;

use pkgcraft::dep::Cpv;
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::shell::BuildPool;

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
    Check { repo, pool: repo.pool() }
}

struct Check {
    repo: &'static EbuildRepo,
    pool: Arc<BuildPool>,
}

impl CpvCheck for Check {
    fn run(&self, cpv: &Cpv, filter: &mut ReportFilter) {
        if let Err(e) = self.pool.metadata(self.repo, cpv, false, false) {
            match e {
                InvalidPkg { id: _, err } => MetadataError.version(cpv).message(err).report(filter),
                _ => unreachable!("{cpv}: unhandled metadata error: {e}"),
            }
        }
    }
}
