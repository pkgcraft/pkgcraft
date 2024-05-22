use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::{EbuildPackage, Pkg};
use pkgcraft::repo::{ebuild::Repo, PkgRepository};

use crate::report::{
    Report,
    ReportKind::{self, MissingSlotDep},
};

pub(super) static REPORTS: &[ReportKind] = &[MissingSlotDep];

#[derive(Debug)]
pub(crate) struct Check<'a> {
    repo: &'a Repo,
}

impl<'a> Check<'a> {
    pub(super) fn new(repo: &'a Repo) -> Self {
        Self { repo }
    }
}

impl<'a> super::CheckRun<&Pkg<'a>> for Check<'a> {
    fn run<F: FnMut(Report)>(&self, pkg: &Pkg<'a>, mut report: F) {
        for dep in pkg
            .rdepend()
            .intersection(pkg.depend())
            .flat_map(|x| x.iter_flatten())
        {
            if dep.blocker().is_none() && dep.slot_dep().is_none() {
                // TODO: use cached lookup instead of searching for each dep
                let slots: IndexSet<_> = self
                    .repo
                    .iter_restrict(dep.no_use_deps().as_ref())
                    .map(|pkg| pkg.slot().to_string())
                    .collect();
                if slots.len() > 1 {
                    let message =
                        format!("{dep} matches multiple slots: {}", slots.iter().join(", "));
                    report(MissingSlotDep.version(pkg, message));
                }
            }
        }
    }
}
