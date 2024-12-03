use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};

use crate::report::ReportKind::CategoryEmpty;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, RepoCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::RepoLayout,
    scope: Scope::Repo,
    source: SourceKind::Repo,
    reports: &[CategoryEmpty],
    context: &[],
    priority: 0,
};

pub(super) fn create() -> impl RepoCheck {
    Check
}

struct Check;

impl RepoCheck for Check {
    fn run(&self, repo: &EbuildRepo, filter: &mut ReportFilter) {
        for category in repo.categories() {
            if repo.packages(&category).is_empty() {
                CategoryEmpty.category(&category).report(filter);
            }
        }
    }
}
