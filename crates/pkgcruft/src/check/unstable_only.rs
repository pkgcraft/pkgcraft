use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::Cpn;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::pkg::ebuild::keyword::{Arch, KeywordStatus::Unstable};
use pkgcraft::restrict::Scope;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::ReportKind::UnstableOnly;
use crate::scan::ScannerRun;
use crate::source::SourceKind;

use super::Context::Optional;

super::register! {
    kind: super::CheckKind::UnstableOnly,
    reports: &[UnstableOnly],
    scope: Scope::Package,
    sources: &[SourceKind::EbuildPkg],
    context: &[Optional],
    create,
}

pub(super) fn create(run: &ScannerRun) -> crate::Result<super::Runner> {
    Ok(Box::new(Check {
        stable: run
            .repo
            .metadata()
            .arches_desc()
            .get("stable")
            .cloned()
            .unwrap_or_default(),
    }))
}

struct Check {
    stable: IndexSet<Arch>,
}

impl super::CheckRun for Check {
    fn run_ebuild_pkg_set(&self, cpn: &Cpn, pkgs: &[EbuildPkg], run: &ScannerRun) {
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
