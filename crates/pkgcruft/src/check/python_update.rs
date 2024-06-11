use std::collections::HashSet;

use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::Flatten;
use pkgcraft::pkg::ebuild::metadata::Key::{self, BDEPEND, DEPEND};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::PkgRepository;
use strum::{AsRefStr, EnumIter, IntoEnumIterator};

use crate::report::ReportKind::PythonUpdate;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;
use crate::utils::use_starts_with;

use super::{CheckKind, VersionCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::PythonUpdate,
    scope: Scope::Version,
    source: SourceKind::Ebuild,
    reports: &[PythonUpdate],
    context: &[],
    priority: 0,
};

static IUSE_PREFIX: &str = "python_targets_";
static IUSE_PREFIX_S: &str = "python_single_target_";

/// Supported python eclasses.
#[derive(AsRefStr, EnumIter, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
#[strum(serialize_all = "kebab-case")]
enum Eclass {
    PythonR1,
    PythonSingleR1,
    PythonAnyR1,
}

impl Eclass {
    /// USE_EXPAND targets pulled from the given repo.
    fn targets<'a>(&self, repo: &'a Repo) -> Vec<&'a str> {
        match self {
            Self::PythonR1 => use_expand(repo, "python_targets"),
            Self::PythonSingleR1 => use_expand(repo, "python_single_target"),
            Self::PythonAnyR1 => use_expand(repo, "python_targets"),
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

// TODO: add inherited use_expand support to pkgcraft so running against overlays works
/// Pull USE_EXPAND targets related to a given name from a target repo.
fn use_expand<'a>(repo: &'a Repo, name: &str) -> Vec<&'a str> {
    repo.metadata
        .use_expand()
        .get(name)
        .map(|x| {
            x.keys()
                .filter(|x| x.starts_with("python"))
                .map(|x| x.as_str())
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn create(repo: &'static Repo) -> impl VersionCheck {
    Check { repo }
}

struct Check {
    repo: &'static Repo,
}

super::register!(Check);

impl VersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        let Some(eclass) = Eclass::iter().find(|x| pkg.inherited().contains(x.as_ref())) else {
            return;
        };

        let deps: IndexSet<_> = pkg
            .dependencies(&eclass.keys())
            .into_iter_flatten()
            .filter(|x| x.blocker().is_none())
            .collect();

        // determine the latest supported implementation
        let Some(latest) = deps
            .iter()
            .filter(|x| x.category() == "dev-lang" && x.package() == "python" && x.slot().is_some())
            .map(|x| x.no_use_deps())
            .sorted()
            .last()
            .map(|x| format!("python{}", x.slot().unwrap().replace('.', "_")))
        else {
            return;
        };

        // determine potential targets
        let mut targets = eclass
            .targets(self.repo)
            .into_iter()
            .rev()
            .take_while(|x| *x != latest)
            .collect::<Vec<_>>();

        if targets.is_empty() {
            return;
        }

        // drop targets with missing dependencies
        for pkg in deps
            .iter()
            .filter(|x| use_starts_with(x, &eclass.prefixes()))
            .filter_map(|x| self.repo.iter_restrict(x.no_use_deps()).last())
        {
            let iuse = pkg
                .iuse()
                .iter()
                .filter_map(|x| deprefix(x.flag(), &eclass.prefixes()))
                .collect::<HashSet<_>>();
            targets.retain(|x| iuse.contains(x));
            if targets.is_empty() {
                return;
            }
        }

        let message = targets.iter().rev().join(", ");
        filter.report(PythonUpdate.version(pkg, message));
    }
}
