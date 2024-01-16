use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::pkg::ebuild::keyword::{cmp_arches, KeywordStatus::Unstable};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::{PackageReport, Report, ReportKind};
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{Check, CheckKind, CheckRun};

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::UnstableOnly,
    source: SourceKind::EbuildPackageSet,
    scope: Scope::Package,
    priority: 0,
    reports: &[ReportKind::Package(PackageReport::UnstableOnly)],
};

#[derive(Debug, Clone)]
pub(crate) struct UnstableOnlyCheck<'a> {
    arches: HashSet<&'a str>,
}

impl<'a> UnstableOnlyCheck<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        let arches = if let Some(arches) = repo.metadata().arches_desc().get("stable") {
            arches.iter().map(|s| s.as_str()).collect()
        } else {
            Default::default()
        };
        Self { arches }
    }
}

impl<'a> CheckRun<Vec<Pkg<'a>>> for UnstableOnlyCheck<'a> {
    fn run(&self, pkgs: &Vec<Pkg<'a>>, reports: &mut Vec<Report>) -> crate::Result<()> {
        use PackageReport::*;

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
            let arches = arches.iter().sorted_by(|a, b| cmp_arches(a, b)).join(", ");
            reports.push(UnstableOnly.report(pkgs, arches));
        }

        Ok(())
    }
}
