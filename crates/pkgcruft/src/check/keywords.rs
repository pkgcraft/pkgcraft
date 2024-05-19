use itertools::Itertools;
use once_cell::sync::Lazy;
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::{
    Report,
    ReportKind::{OverlappingKeywords, UnsortedKeywords},
};

use super::{Check, CheckKind, CheckRun, EbuildPkgCheckKind};

pub(super) static CHECK: Lazy<Check> = Lazy::new(|| {
    Check::build(CheckKind::EbuildPkg(EbuildPkgCheckKind::Keywords))
        .reports([OverlappingKeywords, UnsortedKeywords])
});

#[derive(Debug)]
pub(crate) struct KeywordsCheck<'a> {
    _repo: &'a Repo,
}

impl<'a> KeywordsCheck<'a> {
    pub(super) fn new(_repo: &'a Repo) -> Self {
        Self { _repo }
    }
}

impl<'a> CheckRun<&Pkg<'a>> for KeywordsCheck<'a> {
    fn run(&self, pkg: &Pkg<'a>, reports: &mut Vec<Report>) {
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
            let keywords = overlapping
                .iter()
                .map(|keywords| format!("({})", keywords.iter().sorted().join(", ")))
                .join(", ");
            reports.push(OverlappingKeywords.version(pkg, keywords));
        }

        // ignore overlapping keywords when checking order
        let flattened_keywords: OrderedSet<_> =
            keywords_map.values().filter_map(|x| x.first()).collect();
        let mut sorted_keywords = flattened_keywords.clone();
        sorted_keywords.sort();

        if sorted_keywords != flattened_keywords {
            let keywords = pkg.keywords().iter().join(" ");
            reports.push(UnsortedKeywords.version(pkg, keywords));
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::repo::Repository;
    use pkgcraft::test::TEST_DATA;

    use crate::scanner::Scanner;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        let repo = TEST_DATA.repo("qa-primary").unwrap();
        let check_dir = repo.path().join(CHECK.as_ref());
        let scanner = Scanner::new().jobs(1).checks([&*CHECK]);
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
