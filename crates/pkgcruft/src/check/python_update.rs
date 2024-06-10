use std::collections::HashSet;
use std::sync::OnceLock;

use indexmap::IndexSet;
use itertools::Itertools;
use once_cell::sync::Lazy;
use pkgcraft::dep::Flatten;
use pkgcraft::pkg::ebuild::metadata::Key::{BDEPEND, DEPEND};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::PkgRepository;

use crate::report::ReportKind::PythonUpdate;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::SourceKind;

use super::{CheckKind, VersionCheck};

pub(super) static CHECK: super::Check = super::Check {
    kind: CheckKind::PythonUpdate,
    scope: Scope::Version,
    source: SourceKind::Ebuild,
    reports: &[PythonUpdate],
    context: &[],
    priority: 0,
};

static ECLASSES: Lazy<IndexSet<&str>> = Lazy::new(|| {
    ["python-r1", "python-single-r1", "python-any-r1"]
        .into_iter()
        .collect()
});

static IUSE_PREFIX: &str = "python_targets_";
static IUSE_PREFIX_S: &str = "python_single_target_";

fn deprefix<'a>(s: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes.iter().filter_map(|x| s.strip_prefix(x)).next()
}

pub(super) fn create(repo: &'static Repo) -> impl VersionCheck {
    Check {
        repo,
        multi_target: OnceLock::new(),
        single_target: OnceLock::new(),
    }
}

struct Check {
    repo: &'static Repo,
    multi_target: OnceLock<Vec<&'static str>>,
    single_target: OnceLock<Vec<&'static str>>,
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
                    .filter(|x| x.starts_with("python"))
                    .map(|x| x.as_str())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn multi_target(&self) -> &[&str] {
        self.multi_target
            .get_or_init(|| self.use_expand("python_targets"))
    }

    fn single_target(&self) -> &[&str] {
        self.single_target
            .get_or_init(|| self.use_expand("python_single_target"))
    }
}

super::register!(Check);

impl VersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        // TODO: return on multiple matches
        let Some(eclass) = pkg
            .inherited()
            .iter()
            .map(|x| x.name())
            .find(|x| ECLASSES.contains(x))
        else {
            return;
        };

        let (available_targets, keys, prefixes) = match eclass {
            "python-r1" => (self.multi_target(), vec![], vec![IUSE_PREFIX]),
            "python-single-r1" => (self.single_target(), vec![], vec![IUSE_PREFIX, IUSE_PREFIX_S]),
            "python-any-r1" => {
                (self.multi_target(), vec![BDEPEND, DEPEND], vec![IUSE_PREFIX, IUSE_PREFIX_S])
            }
            _ => return,
        };

        let deps: IndexSet<_> = pkg
            .dependencies(&keys)
            .into_iter_flatten()
            .filter(|x| x.blocker().is_none())
            .collect();

        // determine the latest supported python version
        let Some(latest) = deps
            .iter()
            .filter(|x| {
                x.category() == "dev-lang"
                    && x.package() == "python"
                    // ignore python2 deps
                    && x.slot().map(|x| x != "2.7").unwrap_or_default()
            })
            .map(|x| x.no_use_deps())
            .sorted()
            .last()
        else {
            return;
        };

        let latest_target = format!("python{}", latest.slot().unwrap().replace('.', "_"));
        let mut targets = available_targets
            .iter()
            .rev()
            .take_while(|x| *x != &latest_target)
            .copied()
            .collect::<Vec<_>>();

        if targets.is_empty() {
            return;
        }

        for (dep, use_deps) in deps.iter().filter_map(|x| x.use_deps().map(|u| (x, u))) {
            if use_deps
                .iter()
                .any(|x| x.possible() && deprefix(x.flag(), &prefixes).is_some())
            {
                if let Some(pkg) = self.repo.iter_restrict(dep.no_use_deps().as_ref()).last() {
                    let iuse = pkg
                        .iuse()
                        .iter()
                        .filter_map(|x| deprefix(x.flag(), &prefixes))
                        .collect::<HashSet<_>>();
                    targets.retain(|x| iuse.contains(x));
                    if targets.is_empty() {
                        return;
                    }
                }
            }
        }

        let message = targets.iter().rev().join(", ");
        filter.report(PythonUpdate.version(pkg, message));
    }
}
