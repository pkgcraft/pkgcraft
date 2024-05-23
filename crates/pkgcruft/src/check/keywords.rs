use itertools::Itertools;
use pkgcraft::pkg::ebuild::keyword::KeywordStatus::Stable;
use pkgcraft::pkg::{ebuild::Pkg, Package};
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::{
    Report,
    ReportKind::{self, EapiUnstable, OverlappingKeywords, UnsortedKeywords},
};

pub(super) static REPORTS: &[ReportKind] = &[EapiUnstable, OverlappingKeywords, UnsortedKeywords];

#[derive(Debug)]
pub(crate) struct Check<'a> {
    repo: &'a Repo,
}

impl<'a> Check<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self { repo }
    }
}

impl<'a> super::CheckRun<&Pkg<'a>> for Check<'a> {
    fn run<F: FnMut(Report)>(&self, pkg: &Pkg<'a>, mut report: F) {
        let keywords_map = pkg
            .keywords()
            .iter()
            .map(|k| (k.arch(), k))
            .collect::<OrderedMap<_, OrderedSet<_>>>();
        let overlapping = keywords_map
            .values()
            .filter(|keywords| keywords.len() > 1)
            .collect::<Vec<_>>();

        if !overlapping.is_empty() {
            let message = overlapping
                .iter()
                .map(|keywords| format!("({})", keywords.iter().sorted().join(", ")))
                .join(", ");
            report(OverlappingKeywords.version(pkg, message));
        }

        if self
            .repo
            .metadata
            .config
            .eapis_testing
            .contains(pkg.eapi().as_ref())
        {
            let keywords = pkg
                .keywords()
                .iter()
                .filter(|k| k.status() == Stable)
                .collect::<Vec<_>>();
            if !keywords.is_empty() {
                let message = format!(
                    "unstable EAPI {} with stable keywords: {}",
                    pkg.eapi(),
                    keywords.iter().join(" ")
                );
                report(EapiUnstable.version(pkg, message));
            }
        }

        // ignore overlapping keywords when checking order
        let flattened_keywords = keywords_map
            .values()
            .filter_map(|x| x.first())
            .collect::<OrderedSet<_>>();
        let mut sorted_keywords = flattened_keywords.clone();
        sorted_keywords.sort();

        if sorted_keywords != flattened_keywords {
            let message = pkg.keywords().iter().join(" ");
            report(UnsortedKeywords.version(pkg, message));
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::TEST_DATA;
    use pretty_assertions::assert_eq;

    use crate::check::CheckKind::Keywords;
    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    #[test]
    fn check() {
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let check_dir = repo.path().join(Keywords);
        let scanner = Scanner::new().jobs(1).checks([Keywords]);
        let expected = glob_reports!("{check_dir}/*/reports.json");

        // check dir restriction
        let restrict = repo.restrict_from_path(&check_dir).unwrap();
        let reports: Vec<_> = scanner.run(repo, [&restrict]).collect();
        assert_eq!(&reports, &expected);

        // repo restriction
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);
    }
}
