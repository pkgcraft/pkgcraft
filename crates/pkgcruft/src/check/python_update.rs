use std::collections::{HashMap, HashSet};

use dashmap::{mapref::one::Ref, DashMap};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::Flatten;
use pkgcraft::pkg::ebuild::metadata::Key::{self, BDEPEND, DEPEND};
use pkgcraft::pkg::ebuild::{iuse::Iuse, EbuildPkg};
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::Restrict;
use pkgcraft::types::OrderedSet;
use strum::{AsRefStr, Display, EnumIter, IntoEnumIterator};

use crate::report::ReportKind::PythonUpdate;
use crate::scan::ScannerRun;
use crate::utils::{use_expand, use_starts_with};

use super::EbuildPkgCheck;

static IMPL_PKG: &str = "dev-lang/python";
static IUSE_PREFIXES: &[&str] = &["python_targets_", "python_single_target_"];

/// Supported python eclasses.
#[derive(AsRefStr, Display, EnumIter, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
#[strum(serialize_all = "kebab-case")]
enum Eclass {
    PythonR1,
    PythonSingleR1,
    PythonAnyR1,
}

impl Eclass {
    /// USE_EXPAND targets pulled from the given repo.
    fn targets(&self, repo: &EbuildRepo) -> IndexSet<String> {
        match self {
            Self::PythonR1 => use_expand(repo, "python_targets", "python"),
            Self::PythonSingleR1 => use_expand(repo, "python_single_target", "python"),
            Self::PythonAnyR1 => use_expand(repo, "python_targets", "python"),
        }
    }

    /// Dependency variants to pull for matching, with empty lists pulling all deps.
    fn keys(&self) -> impl Iterator<Item = Key> {
        match self {
            Self::PythonR1 => [].iter().copied(),
            Self::PythonSingleR1 => [].iter().copied(),
            Self::PythonAnyR1 => [DEPEND, BDEPEND].iter().copied(),
        }
    }
}

/// Remove a python IUSE prefix from a string.
fn deprefix(s: &str) -> Option<String> {
    IUSE_PREFIXES
        .iter()
        .filter_map(|x| s.strip_prefix(x))
        .map(|x| x.to_string())
        .next()
}

pub(super) fn create(run: &ScannerRun) -> impl EbuildPkgCheck {
    Check {
        repo: run.repo.clone(),
        targets: Eclass::iter().map(|e| (e, e.targets(&run.repo))).collect(),
        dep_iuse: Default::default(),
    }
}

static CHECK: super::Check = super::Check::PythonUpdate;

struct Check {
    repo: EbuildRepo,
    targets: HashMap<Eclass, IndexSet<String>>,
    dep_iuse: DashMap<Restrict, Option<OrderedSet<Iuse>>>,
}

super::register!(Check);

impl Check {
    /// Get the USE_EXPAND targets for an eclass.
    fn targets(&self, eclass: &Eclass) -> &IndexSet<String> {
        self.targets
            .get(eclass)
            .unwrap_or_else(|| unreachable!("missing eclass targets: {eclass}"))
    }

    /// Get the package IUSE matching a given dependency.
    fn get_dep_iuse<R: Into<Restrict>>(
        &self,
        dep: R,
    ) -> Ref<Restrict, Option<OrderedSet<Iuse>>> {
        let restrict = dep.into();
        self.dep_iuse
            .entry(restrict.clone())
            .or_insert_with(|| {
                self.repo
                    .iter_restrict(restrict)
                    .filter_map(Result::ok)
                    .last()
                    .map(|pkg| pkg.iuse().clone())
            })
            .downgrade()
    }
}

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, run: &ScannerRun) {
        let Some(eclass) = Eclass::iter().find(|x| pkg.inherited().contains(x.as_ref()))
        else {
            return;
        };

        let deps = pkg
            .dependencies(eclass.keys())
            .into_iter_flatten()
            .filter(|x| x.blocker().is_none())
            .collect::<IndexSet<_>>();

        // determine the latest supported implementation
        let Some(latest) = deps
            .iter()
            .filter(|x| x.cpn() == IMPL_PKG)
            .filter_map(|x| x.slot().map(|s| format!("python{}", s.replace('.', "_"))))
            .sorted_by_key(|x| self.targets(&eclass).get_index_of(x.as_str()))
            .last()
        else {
            // missing deps
            return;
        };

        // determine potential targets
        let mut targets = self
            .targets(&eclass)
            .into_iter()
            .rev()
            .take_while(|x| *x != &latest)
            .collect::<Vec<_>>();

        if targets.is_empty() {
            // no updates available
            return;
        }

        // drop targets with missing dependencies
        for dep in deps.iter().filter(|x| use_starts_with(x, IUSE_PREFIXES)) {
            if let Some(iuse) = self.get_dep_iuse(dep.no_use_deps()).as_ref() {
                let iuse = iuse
                    .iter()
                    .filter_map(|x| deprefix(x.flag()))
                    .collect::<HashSet<_>>();
                targets.retain(|x| iuse.contains(x.as_str()));
                if targets.is_empty() {
                    // no updates available
                    return;
                }
            }
        }

        PythonUpdate
            .version(pkg)
            .message(targets.iter().rev().join(", "))
            .report(run);
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use crate::scan::Scanner;
    use crate::source::PkgFilter;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        let filter: PkgFilter = "category != 'stub'".parse().unwrap();
        let scanner = Scanner::new().reports([CHECK]).filters([filter]);

        // gentoo unfixed
        let data = test_data();
        let repo = data.ebuild_repo("gentoo").unwrap();
        let dir = repo.path().join(CHECK);
        // ignore stub/* ebuilds
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // gentoo fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("gentoo").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
