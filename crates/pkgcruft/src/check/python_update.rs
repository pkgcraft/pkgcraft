use std::collections::HashSet;
use std::sync::OnceLock;

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use pkgcraft::dep::Flatten;
use pkgcraft::pkg::ebuild::metadata::Key::{self, BDEPEND, DEPEND};
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::repo::PkgRepository;
use strum::{AsRefStr, Display, EnumIter, IntoEnumIterator};

use crate::report::ReportKind::PythonUpdate;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;
use crate::utils::{use_expand, use_starts_with};

use super::{CheckContext, CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::PythonUpdate,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[PythonUpdate],
    context: &[CheckContext::GentooInherited],
    priority: 0,
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
    fn targets<'a>(&self, repo: &'a EbuildRepo) -> IndexSet<&'a str> {
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
fn deprefix<'a>(s: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes.iter().filter_map(|x| s.strip_prefix(x)).next()
}

pub(super) fn create(repo: &'static EbuildRepo) -> impl EbuildPkgCheck {
    Check {
        repo,
        targets: OnceLock::new(),
        keys: OnceLock::new(),
        prefixes: OnceLock::new(),
    }
}

struct Check {
    repo: &'static EbuildRepo,
    targets: OnceLock<IndexMap<Eclass, IndexSet<&'static str>>>,
    keys: OnceLock<IndexMap<Eclass, Vec<Key>>>,
    prefixes: OnceLock<IndexMap<Eclass, Vec<&'static str>>>,
}

impl Check {
    fn targets(&self, eclass: &Eclass) -> &IndexSet<&'static str> {
        self.targets
            .get_or_init(|| Eclass::iter().map(|e| (e, e.targets(self.repo))).collect())
            .get(eclass)
            .unwrap_or_else(|| panic!("missing eclass targets: {eclass}"))
    }

    fn keys(&self, eclass: &Eclass) -> &[Key] {
        self.keys
            .get_or_init(|| Eclass::iter().map(|e| (e, e.keys())).collect())
            .get(eclass)
            .unwrap_or_else(|| panic!("missing eclass keys: {eclass}"))
    }

    fn prefixes(&self, eclass: &Eclass) -> &[&'static str] {
        self.prefixes
            .get_or_init(|| Eclass::iter().map(|e| (e, e.prefixes())).collect())
            .get(eclass)
            .unwrap_or_else(|| panic!("missing eclass prefixes: {eclass}"))
    }
}

super::register!(Check);

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
        let Some(eclass) = Eclass::iter().find(|x| pkg.inherited().contains(x.as_ref())) else {
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
            .copied()
            .collect::<Vec<_>>();

        if targets.is_empty() {
            // no updates available
            return;
        }

        // drop targets with missing dependencies
        for pkg in deps
            .iter()
            .filter(|x| use_starts_with(x, self.prefixes(&eclass)))
            .filter_map(|x| {
                self.repo
                    .iter_restrict(x.no_use_deps())
                    .filter_map(Result::ok)
                    .last()
            })
        {
            let iuse = pkg
                .iuse()
                .iter()
                .filter_map(|x| deprefix(x.flag(), self.prefixes(&eclass)))
                .collect::<HashSet<_>>();
            targets.retain(|x| iuse.contains(x));
            if targets.is_empty() {
                // no updates available
                return;
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
    use pkgcraft::repo::Repository;
    use pkgcraft::test::{assert_unordered_eq, test_data_patched, TEST_DATA};

    use crate::scanner::Scanner;
    use crate::source::PkgFilter;
    use crate::test::glob_reports;

    use super::*;

    #[test]
    fn check() {
        // gentoo unfixed
        let (pool, repo) = TEST_DATA.repo("gentoo").unwrap();
        let dir = repo.path().join(CHECK);
        // ignore stub/* ebuilds
        let filter: PkgFilter = "category != 'stub'".parse().unwrap();
        let scanner = Scanner::new(&pool)
            .checks([CHECK])
            .filters([filter.clone()]);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, expected);

        // gentoo fixed
        let data = test_data_patched();
        let (pool, repo) = data.repo("gentoo").unwrap();
        let scanner = Scanner::new(&pool)
            .checks([CHECK])
            .filters([filter.clone()]);
        let reports = scanner.run(repo, repo);
        assert_unordered_eq!(reports, []);
    }
}
