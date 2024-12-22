use std::collections::HashSet;
use std::sync::OnceLock;

use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::Flatten;
use pkgcraft::pkg::{ebuild::EbuildPkg, Package};
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::repo::PkgRepository;

use crate::report::ReportKind::RubyUpdate;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;
use crate::utils::{use_expand, use_starts_with};

use super::{CheckContext, CheckKind, EbuildPkgCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::RubyUpdate,
    scope: Scope::Version,
    source: SourceKind::EbuildPkg,
    reports: &[RubyUpdate],
    context: &[CheckContext::GentooInherited],
};

static IUSE_PREFIX: &str = "ruby_targets_";
static IMPL_PKG: &str = "dev-lang/ruby";

pub(super) fn create(repo: &EbuildRepo) -> impl EbuildPkgCheck {
    Check {
        repo: repo.clone(),
        targets: OnceLock::new(),
    }
}

struct Check {
    repo: EbuildRepo,
    targets: OnceLock<IndexSet<String>>,
}

impl Check {
    fn targets(&self) -> &IndexSet<String> {
        self.targets
            .get_or_init(|| use_expand(&self.repo, "ruby_targets", "ruby"))
    }
}

impl EbuildPkgCheck for Check {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter) {
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
            .sorted_by_key(|x| self.targets().get_index_of(x.as_str()))
            .last()
        else {
            // missing deps
            return;
        };

        // determine potential targets
        let mut targets = self
            .targets()
            .iter()
            .rev()
            .take_while(|x| *x != &latest)
            .collect::<Vec<_>>();

        if targets.is_empty() {
            // no updates available
            return;
        }

        // drop targets with missing dependencies
        for pkg in deps
            .iter()
            .filter(|x| use_starts_with(x, &[IUSE_PREFIX]))
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
                .filter_map(|x| x.flag().strip_prefix(IUSE_PREFIX))
                .collect::<HashSet<_>>();
            targets.retain(|x| iuse.contains(x.as_str()));
            if targets.is_empty() {
                // no updates available
                return;
            }
        }

        RubyUpdate
            .version(pkg)
            .message(targets.iter().rev().join(", "))
            .report(filter);
    }
}

#[cfg(test)]
mod tests {
    use pkgcraft::test::*;

    use crate::scanner::Scanner;
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
