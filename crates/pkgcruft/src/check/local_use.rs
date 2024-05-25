use std::collections::HashSet;

use itertools::Itertools;
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;

use crate::report::{
    Report,
    ReportKind::{self, UseLocalDescMissing, UseLocalUnused},
};

pub(super) static REPORTS: &[ReportKind] = &[UseLocalDescMissing, UseLocalUnused];

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
        let mut missing_desc = vec![];

        for (flag, desc) in local_use {
            if desc.is_empty() {
                missing_desc.push(flag);
            }
        }

        if !missing_desc.is_empty() {
            missing_desc.sort();
            let message = missing_desc.iter().join(", ");
            report(UseLocalDescMissing.package(pkgs, message));
        }

        let used = pkgs
            .iter()
            .flat_map(|pkg| pkg.iuse())
            .map(|iuse| iuse.flag())
            .collect::<HashSet<_>>();
        let unused = local_use
            .keys()
            .filter(|x| !used.contains(x.as_str()))
            .sorted()
            .collect::<Vec<_>>();

        if !unused.is_empty() {
            let message = unused.iter().join(", ");
            report(UseLocalUnused.package(pkgs, message));
        }
    }
}
