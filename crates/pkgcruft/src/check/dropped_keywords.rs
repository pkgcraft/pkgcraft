use std::collections::{HashMap, HashSet};

use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::keyword::{cmp_arches, KeywordStatus::Disabled};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{PackageReport, Report, ReportKind};
use crate::source::SourceKind;

use super::{Check, CheckKind, CheckRun, Scope};

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::DroppedKeywords,
    source: SourceKind::EbuildPackageSet,
    scope: Scope::PackageSet,
    priority: 0,
    reports: &[ReportKind::Package(PackageReport::DroppedKeywords)],
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

impl<'a> CheckRun<Vec<Pkg<'a>>> for DroppedKeywordsCheck<'a> {
    fn run(&self, pkgs: &Vec<Pkg<'a>>, reports: &mut Vec<Report>) -> crate::Result<()> {
        use PackageReport::*;

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
            for pkg in pkgs {
                dropped.entry(pkg).or_default().push(arch);
            }
        }

        for (pkg, arches) in &dropped {
            let arches = arches.iter().sorted_by(|a, b| cmp_arches(a, b)).join(", ");
            reports.push(DroppedKeywords.report(pkg, arches));
        }

        Ok(())
    }
}
