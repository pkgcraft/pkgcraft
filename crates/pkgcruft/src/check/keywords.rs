use itertools::Itertools;
use pkgcraft::pkg::ebuild::keyword::KeywordStatus::Stable;
use pkgcraft::pkg::{ebuild::Pkg, Package};
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::ReportKind::{EapiUnstable, KeywordsOverlapping, KeywordsUnsorted};
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, VersionCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::Keywords,
    scope: Scope::Version,
    source: SourceKind::Ebuild,
    reports: &[EapiUnstable, KeywordsOverlapping, KeywordsUnsorted],
    context: &[],
    priority: 0,
};

pub(super) fn create(repo: &'static Repo) -> impl VersionCheck {
    Check { repo }
}

struct Check {
    repo: &'static Repo,
}

super::register!(Check);

impl VersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        let keywords_map = pkg
            .keywords()
            .iter()
            .map(|k| (k.arch(), k))
            .collect::<OrderedMap<_, OrderedSet<_>>>();

        for keywords in keywords_map.values().filter(|k| k.len() > 1) {
            let message = keywords.iter().sorted().join(", ");
            filter.report(KeywordsOverlapping.version(pkg, message));
        }

        let eapi = pkg.eapi().as_str();
        if self.repo.metadata.config.eapis_testing.contains(eapi) {
            let keywords = pkg
                .keywords()
                .iter()
                .filter(|k| k.status() == Stable)
                .sorted()
                .join(" ");
            if !keywords.is_empty() {
                let message = format!("unstable EAPI {eapi} with stable keywords: {keywords}");
                filter.report(EapiUnstable.version(pkg, message));
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
            let message = format!("unsorted KEYWORD: {unsorted} (sorted: {sorted})");
            filter.report(KeywordsUnsorted.version(pkg, message));
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{TEST_DATA, TEST_DATA_PATCHED};
    use pretty_assertions::assert_eq;

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // primary unfixed
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let dir = repo.path().join(CHECK);
        let scanner = Scanner::new().jobs(1).checks([CHECK]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}
