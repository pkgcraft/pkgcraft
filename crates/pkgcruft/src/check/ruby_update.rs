use std::collections::HashSet;
use std::sync::OnceLock;

use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::{DepField, Flatten};
use pkgcraft::pkg::{ebuild::Pkg, Package};
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::PkgRepository;

use crate::report::ReportKind::RubyUpdate;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;
use crate::utils::{use_expand, use_starts_with};

use super::{CheckContext, CheckKind, VersionCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::RubyUpdate,
    scope: Scope::Version,
    source: SourceKind::Ebuild,
    reports: &[RubyUpdate],
    context: &[CheckContext::GentooInherited],
    priority: 0,
};

static IUSE_PREFIX: &str = "ruby_targets_";
static IMPL_PKG: &str = "dev-lang/ruby";

pub(super) fn create(repo: &'static Repo) -> impl VersionCheck {
    Check { repo, targets: OnceLock::new() }
}

struct Check {
    repo: &'static Repo,
    targets: OnceLock<Vec<&'static str>>,
}

impl Check {
    fn targets(&self) -> &[&str] {
        self.targets
            .get_or_init(|| use_expand(self.repo, "ruby_targets", "ruby"))
    }
}

super::register!(Check);

impl VersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        if pkg.category() == "virtual" || !pkg.inherited().contains("ruby-ng") {
            return;
        };

        let deps: IndexSet<_> = pkg
            .dependencies(&[])
            .into_iter_flatten()
            .filter(|x| x.blocker().is_none())
            .collect();

        // determine the latest supported implementation
        let Some(latest) = deps
            .iter()
            .filter(|x| x.cpn() == IMPL_PKG && x.slot().is_some())
            .filter_map(|x| x.without([DepField::Version, DepField::UseDeps]).ok())
            .sorted()
            .last()
            .map(|x| format!("ruby{}", x.slot().unwrap().replace('.', "")))
        else {
            return;
        };

        // determine potential targets
        let mut targets = self
            .targets()
            .iter()
            .rev()
            .take_while(|x| *x != &latest)
            .copied()
            .collect::<Vec<_>>();

        if targets.is_empty() {
            return;
        }

        // drop targets with missing dependencies
        for pkg in deps
            .iter()
            .filter(|x| use_starts_with(x, &[IUSE_PREFIX]))
            .filter_map(|x| self.repo.iter_restrict(x.no_use_deps()).last())
        {
            let iuse = pkg
                .iuse()
                .iter()
                .filter_map(|x| x.flag().strip_prefix(IUSE_PREFIX))
                .collect::<HashSet<_>>();
            targets.retain(|x| iuse.contains(x));
            if targets.is_empty() {
                return;
            }
        }

        let message = targets.iter().rev().join(", ");
        filter.report(RubyUpdate.version(pkg, message));
    }
}
