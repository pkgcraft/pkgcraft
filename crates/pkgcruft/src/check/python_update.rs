use std::collections::HashSet;
use std::sync::OnceLock;

use dashmap::{mapref::one::Ref, DashMap};
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use pkgcraft::dep::Flatten;
use pkgcraft::pkg::ebuild::metadata::Key::{self, BDEPEND, DEPEND};
use pkgcraft::pkg::ebuild::{iuse::Iuse, EbuildPkg};
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::{Restrict, Scope};
use pkgcraft::types::OrderedSet;
use strum::{AsRefStr, Display, EnumIter, IntoEnumIterator};

use crate::iter::ReportFilter;
use crate::report::ReportKind::PythonUpdate;
use crate::source::SourceKind;
use crate::utils::{use_expand, use_starts_with};

use super::{CheckContext, CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::PythonUpdate,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[PythonUpdate],
    context: &[CheckContext::GentooInherited],
};

static IUSE_PREFIX: &str = "python_targets_";
static IUSE_PREFIX_S: &str = "python_single_target_";
static IMPL_PKG: &str = "dev-lang/python";

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
    fn keys(&self) -> Vec<Key> {
        match self {
            Self::PythonR1 => vec![],
            Self::PythonSingleR1 => vec![],
            Self::PythonAnyR1 => vec![DEPEND, BDEPEND],
        }
    }

    /// USE flag dependency prefixes.
    fn prefixes(&self) -> Vec<&'static str> {
        match self {
            Self::PythonR1 => vec![IUSE_PREFIX],
            Self::PythonSingleR1 => vec![IUSE_PREFIX, IUSE_PREFIX_S],
            Self::PythonAnyR1 => vec![IUSE_PREFIX, IUSE_PREFIX_S],
        }
    }
}

/// Remove a prefix from a string, given a list of prefixes.
fn deprefix<S: AsRef<str>>(s: &str, prefixes: &[S]) -> Option<String> {
    prefixes
        .iter()
        .filter_map(|x| s.strip_prefix(x.as_ref()))
        .map(|x| x.to_string())
        .next()
}

pub(super) fn create(repo: &EbuildRepo) -> impl EbuildPkgCheck {
    Check {
        repo: repo.clone(),
        targets: Default::default(),
        keys: Default::default(),
        prefixes: Default::default(),
        dep_iuse: Default::default(),
    }
}

struct Check {
    repo: EbuildRepo,
    targets: OnceLock<IndexMap<Eclass, IndexSet<String>>>,
    keys: OnceLock<IndexMap<Eclass, Vec<Key>>>,
    prefixes: OnceLock<IndexMap<Eclass, Vec<&'static str>>>,
    dep_iuse: DashMap<Restrict, Option<OrderedSet<Iuse>>>,
}

super::register!(Check);

impl Check {
    fn targets(&self, eclass: &Eclass) -> &IndexSet<String> {
        self.targets
            .get_or_init(|| Eclass::iter().map(|e| (e, e.targets(&self.repo))).collect())
            .get(eclass)
            .unwrap_or_else(|| unreachable!("missing eclass targets: {eclass}"))
    }

    fn keys(&self, eclass: &Eclass) -> impl Iterator<Item = Key> + '_ {
        self.keys
            .get_or_init(|| Eclass::iter().map(|e| (e, e.keys())).collect())
            .get(eclass)
            .unwrap_or_else(|| unreachable!("missing eclass keys: {eclass}"))
            .iter()
            .copied()
    }

    fn prefixes(&self, eclass: &Eclass) -> &[&'static str] {
        self.prefixes
            .get_or_init(|| Eclass::iter().map(|e| (e, e.prefixes())).collect())
            .get(eclass)
            .unwrap_or_else(|| unreachable!("missing eclass prefixes: {eclass}"))
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
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        let Some(eclass) = Eclass::iter().find(|x| pkg.inherited().contains(x.as_ref()))
        else {
            return;
        };

        let deps = pkg
            .dependencies(self.keys(&eclass))
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
        for dep in deps
            .iter()
            .filter(|x| use_starts_with(x, self.prefixes(&eclass)))
        {
            if let Some(iuse) = self.get_dep_iuse(dep.no_use_deps()).as_ref() {
                let iuse = iuse
                    .iter()
                    .filter_map(|x| deprefix(x.flag(), self.prefixes(&eclass)))
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
            .report(filter);
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
        // gentoo unfixed
        let data = test_data();
        let repo = data.ebuild_repo("gentoo").unwrap();
        let dir = repo.path().join(CHECK);
        // ignore stub/* ebuilds
        let filter: PkgFilter = "category != 'stub'".parse().unwrap();
        let scanner = Scanner::new(repo).checks([CHECK]).filters([filter.clone()]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, expected);

        // gentoo fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("gentoo").unwrap();
        let scanner = Scanner::new(repo).checks([CHECK]).filters([filter.clone()]);
        let reports = scanner.run(repo).unwrap();
        assert_unordered_eq!(reports, []);
    }
}
