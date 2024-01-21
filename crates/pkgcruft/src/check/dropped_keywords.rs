use std::collections::{HashMap, HashSet};

use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::keyword::{cmp_arches, KeywordStatus::Disabled};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{Report, ReportKind, VersionReport};
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{Check, CheckKind, CheckRun, EbuildPkgSetCheckKind};

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::EbuildPkgSet(EbuildPkgSetCheckKind::DroppedKeywords),
    source: SourceKind::EbuildPackage,
    scope: Scope::Package,
    priority: 0,
    reports: &[ReportKind::Version(VersionReport::DroppedKeywords)],
};

#[derive(Debug, Clone)]
pub(crate) struct DroppedKeywordsCheck<'a> {
    arches: &'a IndexSet<String>,
}

impl<'a> DroppedKeywordsCheck<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self { arches: repo.arches() }
    }
}

impl<'a> CheckRun<&[Pkg<'a>]> for DroppedKeywordsCheck<'a> {
    fn run(&self, pkgs: &[Pkg<'a>], reports: &mut Vec<Report>) -> crate::Result<()> {
        use VersionReport::*;

        // ignore packages lacking keywords
        let pkgs: Vec<_> = pkgs.iter().filter(|p| !p.keywords().is_empty()).collect();
        if pkgs.len() <= 1 {
            return Ok(());
        };

        let mut seen = HashSet::new();
        let mut previous = HashSet::new();
        let mut changes = HashMap::<_, Vec<_>>::new();

        for pkg in &pkgs {
            let arches: HashSet<_> = pkg.keywords().iter().map(|k| k.arch()).collect();

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
                    changes.entry(arch.to_string()).or_default().push(pkg);
                }
            }

            // ignore missing arches on previous versions that were re-enabled
            if !changes.is_empty() {
                let disabled: HashSet<_> = pkg
                    .keywords()
                    .iter()
                    .filter(|k| k.status() == Disabled)
                    .map(|k| k.arch())
                    .collect();
                let adds: HashSet<_> = arches.difference(&previous).copied().collect();
                for arch in adds.difference(&disabled) {
                    changes.remove(*arch);
                }
            }

            seen.extend(arches.clone());
            previous = arches;
        }

        #[allow(clippy::mutable_key_type)] // false positive due to ebuild pkg OnceLock usage
        let mut dropped = HashMap::<_, Vec<_>>::new();
        for (arch, pkgs) in &changes {
            // TODO: report all pkgs with dropped keywords in verbose mode?
            // only report the latest pkg with dropped keywords
            let pkg = pkgs.last().unwrap();
            dropped.entry(pkg).or_default().push(arch);
        }

        for (pkg, arches) in &dropped {
            let arches = arches.iter().sorted_by(|a, b| cmp_arches(a, b)).join(", ");
            reports.push(DroppedKeywords.report(pkg, arches));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::dep::Dep;
    use pkgcraft::repo::{PkgRepository, Repository};
    use pkgcraft::test::TEST_DATA;

    use crate::report::Iter;

    use super::*;

    #[test]
    fn check() {
        let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
        let check = DroppedKeywordsCheck::new(repo);
        let json = repo
            .path()
            .join("DroppedKeywords/DroppedKeywords/reports.json");
        let expected: Result<Vec<_>, _> = Iter::try_from_file(&json).unwrap().collect();
        let dep = Dep::try_new_cpn("DroppedKeywords/DroppedKeywords").unwrap();
        let pkgs: Vec<_> = repo.iter_restrict(&dep).collect();
        let mut reports = vec![];
        check.run(&pkgs, &mut reports).unwrap();
        assert_eq!(&reports, &expected.unwrap());
    }
}
