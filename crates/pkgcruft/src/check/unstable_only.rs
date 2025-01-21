use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::keyword::{Arch, KeywordStatus::Unstable};
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::ReportKind::UnstableOnly;
use crate::scan::ScannerRun;

use super::EbuildPkgSetCheck;

pub(super) fn create(run: &ScannerRun) -> impl EbuildPkgSetCheck {
    Check {
        stable: run
            .repo
            .metadata()
            .arches_desc()
            .get("stable")
            .cloned()
            .unwrap_or_default(),
    }
}

static CHECK: super::Check = super::Check::UnstableOnly;

struct Check {
    stable: IndexSet<Arch>,
}

super::register!(Check);

impl EbuildPkgSetCheck for Check {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildPkg], run: &ScannerRun) {
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
            UnstableOnly.package(cpn).message(arches).report(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::{test_data, test_data_patched};

    use crate::scan::Scanner;
    use crate::test::{assert_unordered_reports, glob_reports};

    use super::*;

    #[test]
    fn check() {
        // unselected
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new();
        let reports = scanner.run(repo, &dir).unwrap();
        assert_unordered_reports!(reports, []);

        let scanner = Scanner::new().reports([CHECK]);

        // primary unfixed
        let expected = glob_reports!("{dir}/*/optional.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
