use dashmap::DashSet;
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use pkgcraft::pkg::ebuild::{keyword::KeywordStatus::Stable, EbuildPkg};
use pkgcraft::pkg::Package;

use crate::report::ReportKind::{
    ArchesUnused, EapiUnstable, KeywordsLive, KeywordsOverlapping, KeywordsUnsorted,
};
use crate::scan::ScannerRun;

use super::EbuildPkgCheck;

pub(super) fn create(run: &ScannerRun) -> impl EbuildPkgCheck + 'static {
    let unused = if run.enabled(ArchesUnused) {
        run.repo
            .metadata()
            .arches()
            .iter()
            .map(|x| x.to_string())
            .collect()
    } else {
        Default::default()
    };

    Check { unused }
}

static CHECK: super::Check = super::Check::Keywords;

struct Check {
    unused: DashSet<String>,
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, run: &ScannerRun) {
        if !pkg.keywords().is_empty() && pkg.live() {
            KeywordsLive
                .version(pkg)
                .message(pkg.keywords().iter().join(", "))
                .report(run);
        }

        let mut keywords_map = IndexMap::<_, IndexSet<_>>::new();
        for k in pkg.keywords() {
            // mangle values for post-run finalization
            if run.enabled(ArchesUnused) {
                self.unused.remove(k.arch().as_ref());
            }

            keywords_map.entry(k.arch()).or_default().insert(k);
        }

        for keywords in keywords_map.values().filter(|k| k.len() > 1) {
            KeywordsOverlapping
                .version(pkg)
                .message(keywords.iter().sorted().join(", "))
                .report(run);
        }

        let eapi = pkg.eapi().as_str();
        if run.repo.metadata().config.eapis_testing.contains(eapi) {
            let keywords = pkg
                .keywords()
                .iter()
                .filter(|k| k.status() == Stable)
                .sorted()
                .join(" ");
            if !keywords.is_empty() {
                EapiUnstable
                    .version(pkg)
                    .message(format!("unstable EAPI {eapi} with stable keywords: {keywords}"))
                    .report(run);
            }
        }

        // ignore overlapping keywords when checking order
        let unsorted_keywords = keywords_map
            .values()
            .filter_map(|x| x.first())
            .collect::<Vec<_>>();
        let sorted_keywords = unsorted_keywords.iter().sorted().collect::<Vec<_>>();
        let sorted_diff = unsorted_keywords
            .iter()
            .zip(sorted_keywords)
            .find(|(a, b)| a != b);
        if let Some((unsorted, sorted)) = sorted_diff {
            KeywordsUnsorted
                .version(pkg)
                .message(format!("unsorted KEYWORD: {unsorted} (sorted: {sorted})"))
                .report(run);
        }
    }

    fn finish_check(&self, run: &ScannerRun) {
        if run.enabled(ArchesUnused) && !self.unused.is_empty() {
            let unused = self
                .unused
                .iter()
                .map(|x| x.to_string())
                .sorted()
                .join(", ");
            ArchesUnused.repo(&run.repo).message(unused).report(run);
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
        let scanner = Scanner::new().reports([CHECK]);

        // primary unfixed
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let expected = glob_reports!("{dir}/**/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // primary fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
