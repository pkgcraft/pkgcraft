use std::collections::{HashMap, HashSet};

use dashmap::{DashMap, mapref::one::MappedRef};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::Flatten;
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::pkg::ebuild::metadata::Key::{self, BDEPEND, DEPEND};
use pkgcraft::repo::{EbuildRepo, PkgRepository};
use pkgcraft::restrict::{Restrict, Scope};
use strum::{AsRefStr, Display, EnumIter, IntoEnumIterator};

use crate::report::ReportKind::PythonUpdate;
use crate::scan::ScannerRun;
use crate::source::SourceKind;
use crate::utils::{impl_targets, use_starts_with};

use super::Context::GentooInherited;

super::register! {
    kind: super::CheckKind::PythonUpdate,
    reports: &[PythonUpdate],
    scope: Scope::Version,
    sources: &[SourceKind::EbuildPkg],
    context: &[GentooInherited],
    create,
}

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
    fn targets(&self, repo: &EbuildRepo) -> impl Iterator<Item = String> {
        match self {
            Self::PythonR1 => impl_targets(repo, "python_targets", "python"),
            Self::PythonSingleR1 => impl_targets(repo, "python_single_target", "python"),
            Self::PythonAnyR1 => impl_targets(repo, "python_targets", "python"),
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

pub(super) fn create(run: &ScannerRun) -> super::Runner {
    Box::new(Check {
        targets: Eclass::iter()
            .map(|e| (e, e.targets(&run.repo).collect()))
            .collect(),
        dep_targets: Default::default(),
    })
}

struct Check {
    targets: HashMap<Eclass, Vec<String>>,
    dep_targets: DashMap<Restrict, Option<HashSet<String>>>,
}

/// Determine the set of compatible targets for a dependency.
fn dep_targets(pkg: EbuildPkg) -> HashSet<String> {
    pkg.iuse()
        .iter()
        .filter_map(|x| IUSE_PREFIXES.iter().find_map(|s| x.flag().strip_prefix(s)))
        .map(|x| x.to_string())
        .collect()
}

impl Check {
    /// Get the USE_EXPAND targets for an eclass.
    fn targets(&self, eclass: &Eclass) -> &[String] {
        self.targets
            .get(eclass)
            .unwrap_or_else(|| unreachable!("missing eclass targets: {eclass}"))
    }

    /// Get the set of compatible targets for a dependency if they exist.
    fn get_targets<R: Into<Restrict>>(
        &self,
        repo: &EbuildRepo,
        dep: R,
    ) -> Option<MappedRef<'_, Restrict, Option<HashSet<String>>, HashSet<String>>> {
        let restrict = dep.into();
        self.dep_targets
            .entry(restrict.clone())
            .or_insert_with(|| {
                repo.iter_restrict(restrict)
                    .filter_map(Result::ok)
                    .last()
                    .map(dep_targets)
            })
            .downgrade()
            .try_map(|x| x.as_ref())
            .ok()
    }
}

impl super::CheckRun for Check {
    fn run_ebuild_pkg(&self, pkg: &EbuildPkg, run: &ScannerRun) {
        let Some(eclass) = Eclass::iter().find(|x| pkg.inherited().contains(x.as_ref()))
        else {
            return;
        };

        let deps = pkg
            .dependencies(eclass.keys())
            .into_iter_flatten()
            .filter(|x| x.blocker().is_none())
            .collect::<IndexSet<_>>();

        // determine supported implementations
        let supported = deps
            .iter()
            .filter(|x| x.cpn() == IMPL_PKG)
            .filter_map(|x| x.slot().map(|s| format!("python{}", s.replace('.', "_"))))
            .collect::<HashSet<_>>();

        // determine target implementations
        let mut targets = self
            .targets(&eclass)
            .iter()
            .rev()
            .take_while(|&x| !supported.contains(x))
            .collect::<Vec<_>>();

        if targets.is_empty() {
            // no updates available
            return;
        }

        // drop targets with missing dependencies
        for dep in deps
            .iter()
            .filter(|x| use_starts_with(x, IUSE_PREFIXES))
            .map(|x| x.no_use_deps())
        {
            if let Some(dep_targets) = self.get_targets(&run.repo, dep.no_use_deps()) {
                targets.retain(|&x| dep_targets.contains(x));
                if !targets.is_empty() {
                    continue;
                }
            }

            // no updates available
            return;
        }

        PythonUpdate
            .version(pkg)
            .message(targets.iter().rev().join(", "))
            .report(run);
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::{assert_err_re, test_data, test_data_patched};

    use crate::scan::Scanner;
    use crate::source::PkgFilter;
    use crate::test::{assert_unordered_reports, glob_reports};

    use super::*;

    #[test]
    fn check() {
        // ignore stub/* ebuilds
        let filter: PkgFilter = "category != 'stub'".parse().unwrap();
        let scanner = Scanner::new().reports([CHECK]).filters([filter]);

        // check can't run in non-gentoo inheriting repo
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let r = scanner.run(repo, repo);
        assert_err_re!(r, "PythonUpdate: check requires gentoo-inherited context");

        // gentoo unfixed
        let repo = data.ebuild_repo("gentoo").unwrap();
        let dir = repo.path().join(CHECK);
        let expected = glob_reports!("{dir}/*/reports.json");
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, expected);

        // gentoo fixed
        let data = test_data_patched();
        let repo = data.ebuild_repo("gentoo").unwrap();
        let reports = scanner.run(repo, repo).unwrap();
        assert_unordered_reports!(reports, []);
    }
}
