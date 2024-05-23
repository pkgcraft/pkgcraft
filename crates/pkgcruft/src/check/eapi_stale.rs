use pkgcraft::pkg::ebuild::{EbuildPackage, Pkg};
use pkgcraft::pkg::Package;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::traits::Contains;
use pkgcraft::types::{OrderedMap, OrderedSet};

use crate::report::{
    Report,
    ReportKind::{self, EapiStale},
};

pub(super) static REPORTS: &[ReportKind] = &[EapiStale];

#[derive(Debug)]
pub(crate) struct Check<'a> {
    _repo: &'a Repo,
}

impl<'a> Check<'a> {
    pub(super) fn new(_repo: &'a Repo) -> Self {
        Self { _repo }
    }
}

impl<'a> super::CheckRun<&[Pkg<'a>]> for Check<'a> {
    fn run<F: FnMut(Report)>(&self, pkgs: &[Pkg<'a>], mut report: F) {
        pkgs.iter()
            .map(|pkg| (pkg.slot(), pkg))
            .collect::<OrderedMap<_, OrderedSet<_>>>()
            .values()
            .for_each(|pkgs| {
                let (live, release): (Vec<&Pkg>, Vec<&Pkg>) = pkgs
                    .into_iter()
                    .partition(|pkg| pkg.properties().contains("live"));

                if let Some(latest_release) = release.last() {
                    for pkg in live {
                        if pkg.eapi() < latest_release.eapi() {
                            report(EapiStale.version(pkg, pkg.eapi()));
                        }
                    }
                }
            })
    }
}
