use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::keyword::{Arch, KeywordStatus::Unstable};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::ReportKind::UnstableOnly;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckContext, CheckKind, EbuildPkgSetCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::UnstableOnly,
    scope: Scope::Package,
    source: SourceKind::EbuildPkg,
    reports: &[UnstableOnly],
    context: &[CheckContext::Optional],
    priority: 0,
};

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildPkgSetCheck {
    Check {
        stable: repo
            .metadata()
            .arches_desc()
            .get("stable")
            .cloned()
            .unwrap_or_default(),
    }
}

struct Check {
    stable: IndexSet<Arch>,
}

super::register!(Check);

impl EbuildPkgSetCheck for Check {
    fn run(&self, cpn: &Cpn, pkgs: &[Pkg], filter: &mut ReportFilter) {
        let arches = pkgs
            .iter()
            .flat_map(|pkg| pkg.keywords())
            // select keywords allowed stable in the repo
            .filter(|kw| self.stable.contains(kw.arch()))
            .map(|kw| (kw.arch(), kw))
            // collapse keywords into an arch->keyword mapping
            .collect::<OrderedMap<_, OrderedSet<_>>>()
            .into_iter()
            // find arches that only have unstable keywords
            .filter_map(|(k, v)| v.iter().all(|k| k.status() == Unstable).then_some(k))
            .sorted()
            .join(", ");

        if !arches.is_empty() {
            UnstableOnly.package(cpn).message(arches).report(filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_ordered_eq, TEST_DATA, TEST_DATA_PATCHED};

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // unselected
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let restrict = repo.restrict_from_path(&dir).unwrap();
        let scanner = Scanner::new().jobs(1);
        let reports = scanner.run(repo, [restrict]);
        assert_ordered_eq!(reports, []);

        // primary unfixed
        let scanner = Scanner::new().jobs(1).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/optional.json");
        let reports = scanner.run(repo, [repo]);
        assert_ordered_eq!(reports, expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports = scanner.run(repo, [repo]);
        assert_ordered_eq!(reports, []);

        // empty repo
        let repo = TEST_DATA.repo("empty").unwrap();
        let reports = scanner.run(repo, [repo]);
        assert_ordered_eq!(reports, []);
    }
}
