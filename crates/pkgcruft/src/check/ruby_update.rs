use std::collections::HashSet;

use dashmap::{mapref::one::MappedRef, DashMap};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::Flatten;
use pkgcraft::pkg::{ebuild::EbuildPkg, Package};
use pkgcraft::repo::{ebuild::EbuildRepo, PkgRepository};
use pkgcraft::restrict::Restrict;

use crate::report::ReportKind::RubyUpdate;
use crate::scan::ScannerRun;
use crate::utils::{use_expand, use_starts_with};

use super::EbuildPkgCheck;

static IUSE_PREFIX: &str = "ruby_targets_";
static IMPL_PKG: &str = "dev-lang/ruby";

pub(super) fn create(run: &ScannerRun) -> impl EbuildPkgCheck {
    Check {
        targets: use_expand(&run.repo, "ruby_targets", "ruby"),
        dep_targets: Default::default(),
    }
}

static CHECK: super::Check = super::Check::RubyUpdate;

struct Check {
    targets: IndexSet<String>,
    dep_targets: DashMap<Restrict, Option<HashSet<String>>>,
}

super::register!(Check);

/// Determine the set of compatible targets for a dependency.
fn dep_targets(pkg: EbuildPkg) -> HashSet<String> {
    pkg.iuse()
        .iter()
        .filter_map(|x| x.flag().strip_prefix(IUSE_PREFIX))
        .map(|x| x.to_string())
        .collect()
}

impl Check {
    /// Get the set of compatible targets for a dependency if they exist.
    fn get_targets<R: Into<Restrict>>(
        &self,
        repo: &EbuildRepo,
        dep: R,
    ) -> Option<MappedRef<Restrict, Option<HashSet<String>>, HashSet<String>>> {
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

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, run: &ScannerRun) {
        if pkg.category() == "virtual" || !pkg.inherited().contains("ruby-ng") {
            return;
        };

        let deps = pkg
            .dependencies([])
            .into_iter_flatten()
            .filter(|x| x.blocker().is_none())
            .collect::<IndexSet<_>>();

        // determine the latest supported implementation
        let Some(latest) = deps
            .iter()
            .filter(|x| x.cpn() == IMPL_PKG)
            .filter_map(|x| x.slot().map(|s| format!("ruby{}", s.replace('.', ""))))
            .sorted_by_key(|x| self.targets.get_index_of(x))
            .last()
        else {
            // missing deps
            return;
        };

        // determine potential targets
        let mut targets = self
            .targets
            .iter()
            .rev()
            .take_while(|x| *x != &latest)
            .collect::<Vec<_>>();

        if targets.is_empty() {
            // no updates available
            return;
        }

        // drop targets with missing dependencies
        for dep_targets in deps
            .iter()
            .filter(|x| use_starts_with(x, &[IUSE_PREFIX]))
            .filter_map(|dep| self.get_targets(&run.repo, dep.no_use_deps()))
        {
            targets.retain(|&x| dep_targets.contains(x));
            if targets.is_empty() {
                // no updates available
                return;
            }
        }

        RubyUpdate
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
