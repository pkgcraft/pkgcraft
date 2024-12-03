use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};

use crate::report::ReportKind::{RepoCategoryEmpty, RepoPackageEmpty};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, RepoCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::RepoLayout,
    scope: Scope::Repo,
    source: SourceKind::Repo,
    reports: &[RepoCategoryEmpty, RepoPackageEmpty],
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
            let pkgs = repo.packages(&category);
            if pkgs.is_empty() {
                RepoCategoryEmpty.category(&category).report(filter);
            }
            for pkg in pkgs {
                if repo.versions(&category, &pkg).is_empty() {
                    RepoPackageEmpty.package((&category, &pkg)).report(filter);
                }
            }
        }
    }
}
