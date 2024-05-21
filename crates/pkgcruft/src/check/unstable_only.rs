use std::collections::HashSet;

use itertools::Itertools;
use once_cell::sync::Lazy;
use pkgcraft::pkg::ebuild::keyword::{cmp_arches, KeywordStatus::Unstable};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::{Report, ReportKind::UnstableOnly};
use crate::scope::Scope;

use super::{CheckBuilder, CheckKind, CheckRun};

pub(super) static CHECK: Lazy<super::Check> = Lazy::new(|| {
    CheckBuilder::new(CheckKind::UnstableOnly)
        .scope(Scope::Package)
        .reports([UnstableOnly])
});

#[derive(Debug)]
pub(crate) struct Check<'a> {
    arches: HashSet<&'a str>,
}

impl<'a> Check<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        let arches = if let Some(arches) = repo.metadata.arches_desc().get("stable") {
            arches.iter().map(|s| s.as_str()).collect()
        } else {
            Default::default()
        };
        Self { arches }
    }
}

impl<'a> CheckRun<&[Pkg<'a>]> for Check<'a> {
    fn run<F: FnMut(Report)>(&self, pkgs: &[Pkg<'a>], mut report: F) {
        let arches: Vec<_> = pkgs
            .iter()
            .flat_map(|pkg| pkg.keywords())
            // select keywords allowed stable in the repo
            .filter(|kw| self.arches.contains(kw.arch()))
            .map(|kw| (kw.arch(), kw))
            // collapse keywords into an arch->keyword mapping
            .collect::<OrderedMap<_, OrderedSet<_>>>()
            .into_iter()
            // find arches that only have unstable keywords
            .filter(|(_, v)| v.iter().all(|k| k.status() == Unstable))
            .map(|(k, _)| k)
            .collect();

        if !arches.is_empty() {
            let message = arches.iter().sorted_by(|a, b| cmp_arches(a, b)).join(", ");
            report(UnstableOnly.package(pkgs, message));
        }
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::config::Config;
    use pkgcraft::repo::Repository;
    use pkgcraft::test::TEST_DATA;
    use pretty_assertions::assert_eq;

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

        // empty repo
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo();
        assert!(scanner.run(repo, [repo]).next().is_none());
    }
}
