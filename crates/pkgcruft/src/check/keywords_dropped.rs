use std::collections::{HashMap, HashSet};

use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::keyword::{cmp_arches, KeywordStatus::Disabled};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;

use crate::report::ReportKind::KeywordsDropped;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

pub(super) static CHECK: super::Check = super::Check {
    kind: super::CheckKind::KeywordsDropped,
    scope: Scope::Package,
    source: SourceKind::Ebuild,
    reports: &[KeywordsDropped],
    context: &[],
    priority: 0,
};

#[derive(Debug)]
pub(crate) struct Check {
    arches: &'static IndexSet<String>,
}

impl Check {
    pub(crate) fn new(repo: &'static Repo) -> Self {
        Self { arches: repo.arches() }
    }
}

impl super::PackageCheckRun for Check {
    fn run(&self, pkgs: &[Pkg], filter: &mut ReportFilter) {
        // ignore packages lacking keywords
        let pkgs = pkgs
            .iter()
            .filter(|p| !p.keywords().is_empty())
            .collect::<Vec<_>>();
        if pkgs.len() <= 1 {
            return;
        };

        let mut seen = HashSet::new();
        let mut previous = HashSet::new();
        let mut changes = HashMap::<_, _>::new();

        for pkg in &pkgs {
            let arches = pkg
                .keywords()
                .iter()
                .map(|k| k.arch())
                .collect::<HashSet<_>>();

            // globbed arches override all dropped keywords
            let drops = if arches.contains("*") {
                Default::default()
            } else {
                previous
                    .difference(&arches)
                    .chain(seen.difference(&arches))
                    .collect::<HashSet<_>>()
            };

            for arch in drops {
                if self.arches.contains(*arch) {
                    changes.insert(arch.to_string(), pkg);
                }
            }

            // ignore missing arches on previous versions that were re-enabled
            if !changes.is_empty() {
                let disabled = pkg
                    .keywords()
                    .iter()
                    .filter(|k| k.status() == Disabled)
                    .map(|k| k.arch())
                    .collect::<HashSet<_>>();
                let adds = arches
                    .difference(&previous)
                    .copied()
                    .collect::<HashSet<_>>();
                for arch in adds.difference(&disabled) {
                    changes.remove(*arch);
                }
            }

            seen.extend(arches.clone());
            previous = arches;
        }

        #[allow(clippy::mutable_key_type)] // false positive due to ebuild pkg OnceLock usage
        let mut dropped = HashMap::<_, Vec<_>>::new();
        for (arch, pkg) in &changes {
            // TODO: report all pkgs with dropped keywords in verbose mode?
            // only report the latest pkg with dropped keywords
            dropped.entry(pkg).or_default().push(arch);
        }

        for (pkg, arches) in &dropped {
            let message = arches.iter().sorted_by(|a, b| cmp_arches(a, b)).join(", ");
            filter.report(KeywordsDropped.version(pkg, message));
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
        let check_dir = repo.path().join(CHECK);
        let scanner = Scanner::new().jobs(1).checks([CHECK]);
        let expected = glob_reports!("{check_dir}/*/reports.json");
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &expected);

        // primary fixed
        let repo = TEST_DATA_PATCHED.repo("qa-primary").unwrap();
        let reports: Vec<_> = scanner.run(repo, [repo]).collect();
        assert_eq!(&reports, &[]);
    }
}
