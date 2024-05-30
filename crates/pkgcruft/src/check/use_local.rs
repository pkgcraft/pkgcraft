use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{
    Report,
    ReportKind::{self, UseGlobalMatching, UseLocalDescMissing, UseLocalUnsorted, UseLocalUnused},
};

pub(super) static REPORTS: &[ReportKind] =
    &[UseGlobalMatching, UseLocalDescMissing, UseLocalUnused, UseLocalUnsorted];

#[derive(Debug)]
pub(crate) struct Check<'a> {
    repo: &'a Repo,
}

impl<'a> Check<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self { repo }
    }
}

impl<'a> super::CheckRun<&[Pkg<'a>]> for Check<'a> {
    fn run<F: FnMut(Report)>(&self, pkgs: &[Pkg<'a>], mut report: F) {
        let local_use = pkgs[0].local_use();
        let sorted_flags = local_use
            .keys()
            .map(|s| s.as_str())
            .sorted()
            .collect::<Vec<_>>();

        let sorted_diff = local_use.keys().zip(&sorted_flags).find(|(a, b)| a != b);
        if let Some((unsorted, sorted)) = sorted_diff {
            let message = format!("unsorted flag: {unsorted} (sorted: {sorted})");
            report(UseLocalUnsorted.package(pkgs, message));
        }

        for (flag, desc) in local_use {
            if desc.is_empty() {
                report(UseLocalDescMissing.package(pkgs, flag));
            }

            if let Some(global_desc) = self.repo.metadata.use_global().get(flag) {
                if global_desc == desc {
                    report(UseGlobalMatching.package(pkgs, flag));
                }
            }
        }

        let used = pkgs
            .iter()
            .flat_map(|pkg| pkg.iuse())
            .map(|iuse| iuse.flag())
            .collect::<HashSet<_>>();
        let unused = sorted_flags
            .iter()
            .filter(|&x| !used.contains(x))
            .collect::<Vec<_>>();

        if !unused.is_empty() {
            let message = unused.iter().join(", ");
            report(UseLocalUnused.package(pkgs, message));
        }
    }
}
