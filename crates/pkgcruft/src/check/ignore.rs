use itertools::Itertools;
use pkgcraft::repo::ebuild::EbuildRepo;

use crate::iter::ReportFilter;
use crate::report::ReportKind::IgnoreUnused;

use super::RepoCheck;

pub(super) fn create() -> impl RepoCheck {
    Check
}

static CHECK: super::Check = super::Check::Ignore;

struct Check;

super::register!(Check);

impl RepoCheck for Check {
    fn run(&self, _repo: &EbuildRepo, _filter: &ReportFilter) {}
    fn finish(&self, repo: &EbuildRepo, filter: &ReportFilter) {
        if filter.enabled(IgnoreUnused) {
            for (path, sets) in filter.ignore.unused() {
                let sets = sets.iter().join(", ");
                IgnoreUnused
                    .repo(repo)
                    .message(format!("{path}: {sets}"))
                    .report(filter);
            }
        }
    }
}
