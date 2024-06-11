use std::collections::HashSet;
use std::sync::OnceLock;

use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::Flatten;
use pkgcraft::pkg::{ebuild::Pkg, Package};
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::PkgRepository;

use crate::report::ReportKind::RubyUpdate;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;
use crate::utils::use_starts_with;

use super::{CheckKind, VersionCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::RubyUpdate,
    scope: Scope::Version,
    source: SourceKind::Ebuild,
    reports: &[RubyUpdate],
    context: &[],
    priority: 0,
};

static IUSE_PREFIX: &str = "ruby_targets_";

pub(super) fn create(repo: &'static Repo) -> impl VersionCheck {
    Check { repo, targets: OnceLock::new() }
}

struct Check {
    repo: &'static Repo,
    targets: OnceLock<Vec<&'static str>>,
}

impl Check {
    fn use_expand(&self, name: &str) -> Vec<&'static str> {
        // TODO: add inherited use_expand support to pkgcraft so running against overlays works
        self.repo
            .metadata
            .use_expand()
            .get(name)
            .map(|x| {
                x.keys()
                    .filter(|x| x.starts_with("ruby"))
                    .map(|x| x.as_str())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn targets(&self) -> &[&str] {
        self.targets.get_or_init(|| self.use_expand("ruby_targets"))
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

        // determine the latest supported python version
        let Some(latest) = deps
            .iter()
            .filter(|x| x.category() == "dev-lang" && x.package() == "ruby" && x.slot().is_some())
            .map(|x| x.no_use_deps())
            .sorted()
            .last()
        else {
            return;
        };

        let latest_target = format!("ruby{}", latest.slot().unwrap().replace('.', ""));
        let mut targets = self
            .targets()
            .iter()
            .rev()
            .take_while(|x| *x != &latest_target)
            .copied()
            .collect::<Vec<_>>();

        if targets.is_empty() {
            return;
        }

        for dep in deps.iter().filter(|x| use_starts_with(x, &[IUSE_PREFIX])) {
            if let Some(pkg) = self.repo.iter_restrict(dep.no_use_deps()).last() {
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
        }

        let message = targets.iter().rev().join(", ");
        filter.report(RubyUpdate.version(pkg, message));
    }
}
