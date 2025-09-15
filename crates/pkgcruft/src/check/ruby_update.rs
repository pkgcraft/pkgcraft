use std::collections::HashSet;

use dashmap::{DashMap, mapref::one::MappedRef};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::Flatten;
use pkgcraft::pkg::{Package, ebuild::EbuildPkg};
use pkgcraft::repo::{EbuildRepo, PkgRepository};
use pkgcraft::restrict::{Restrict, Scope};

use crate::report::ReportKind::RubyUpdate;
use crate::scan::ScannerRun;
use crate::source::SourceKind;
use crate::utils::{impl_targets, use_starts_with};

use super::Context::GentooInherited;

super::register! {
    super::Check {
        kind: super::CheckKind::RubyUpdate,
        reports: &[RubyUpdate],
        scope: Scope::Version,
        sources: &[SourceKind::EbuildPkg],
        context: &[GentooInherited],
        create,
    }
}

static IUSE_PREFIX: &str = "ruby_targets_";
static IMPL_PKG: &str = "dev-lang/ruby";

pub(super) fn create(run: &ScannerRun) -> super::Runner {
    Box::new(Check {
        targets: impl_targets(&run.repo, "ruby_targets", "ruby").collect(),
        dep_targets: Default::default(),
    })
}

struct Check {
    targets: Vec<String>,
    dep_targets: DashMap<Restrict, Option<HashSet<String>>>,
}

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
        if pkg.category() == "virtual" || !pkg.inherited().contains("ruby-ng") {
            return;
        };

        let deps = pkg
            .dependencies([])
            .into_iter_flatten()
            .filter(|x| x.blocker().is_none())
            .collect::<IndexSet<_>>();

        // determine supported implementations
        let supported = deps
            .iter()
            .filter(|x| x.cpn() == IMPL_PKG)
            .filter_map(|x| x.slot().map(|s| format!("ruby{}", s.replace('.', ""))))
            .collect::<HashSet<_>>();

        // determine target implementations
        let mut targets = self
            .targets
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
            .filter(|x| use_starts_with(x, &[IUSE_PREFIX]))
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

        RubyUpdate
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
        assert_err_re!(r, "RubyUpdate: check requires gentoo-inherited context");

        // gentoo unfixed
        let data = test_data();
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
