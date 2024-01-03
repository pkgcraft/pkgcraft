use std::collections::{HashMap, HashSet};

use crossbeam_channel::Sender;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::keyword::KeywordStatus::Stable;
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{Report, ReportKind};
use crate::source::SourceKind;

use super::{Check, CheckKind, CheckRun, Scope};

pub struct UnstableOnly {
    category: String,
    package: String,
    arches: Vec<String>,
}

impl std::fmt::Display for UnstableOnly {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}/{}: UnstableOnly: for arches: {}",
            self.category,
            self.package,
            self.arches.iter().join(", "),
        )
    }
}

impl From<UnstableOnly> for Report {
    fn from(value: UnstableOnly) -> Self {
        Self::UnstableOnly(value)
    }
}

pub(crate) static CHECK: Check = Check {
    kind: CheckKind::UnstableOnly,
    source: SourceKind::EbuildPackageSet,
    scope: Scope::PackageSet,
    priority: 0,
    reports: &[ReportKind::UnstableOnly],
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
    fn run(&self, pkgs: &Vec<Pkg<'a>>, tx: &Sender<Report>) -> crate::Result<()> {
        // iterator over arches allowed as stable by the repo
        let stable_keywords = pkgs
            .iter()
            .flat_map(|p| p.keywords())
            .filter(|k| self.arches.contains(k.arch()));

        // collapse keywords into an arch-keyed mapping
        let mut pkg_keywords = HashMap::<_, Vec<_>>::new();
        for k in stable_keywords {
            pkg_keywords.entry(k.arch()).or_default().push(k);
        }

        // find arches that only have unstable keywords
        let unstable: Vec<_> = pkg_keywords
            .iter()
            .filter(|(_, v)| v.iter().all(|k| k.status() != Stable))
            .map(|(k, _)| k)
            .sorted()
            .collect();

        if !unstable.is_empty() {
            let pkg = &pkgs[0];
            tx.send(
                UnstableOnly {
                    category: pkg.cpv().category().to_string(),
                    package: pkg.cpv().package().to_string(),
                    arches: unstable.iter().map(|s| s.to_string()).collect(),
                }
                .into(),
            )
            .unwrap();
        }

        Ok(())
    }
}
