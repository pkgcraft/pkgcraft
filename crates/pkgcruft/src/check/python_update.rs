use std::collections::{HashMap, HashSet};

use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::{Flatten, UseDepKind};
use pkgcraft::pkg::ebuild::metadata::Key;
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::{Eclass, Repo};
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

static ECLASSES: &[&str] = &["python-r1", "python-single-r1", "python-any-r1"];

pub(super) fn create(repo: &'static Repo) -> impl VersionCheck {
    let eclasses = repo
        .eclasses()
        .values()
        .filter(|x| ECLASSES.contains(&x.name()))
        .collect();

    // TODO: add inherited use_expand support to pkgcraft so running against overlays works
    let multi_target = repo
        .metadata
        .use_expand()
        .get("python_targets")
        .map(|x| {
            x.keys()
                .filter(|x| x.starts_with("python"))
                .collect::<IndexSet<_>>()
        })
        .unwrap_or_default();
    let single_target = repo
        .metadata
        .use_expand()
        .get("python_single_target")
        .map(|x| {
            x.keys()
                .filter(|x| x.starts_with("python"))
                .collect::<IndexSet<_>>()
        })
        .unwrap_or_default();

    let params = [
        ("python-r1".to_string(), (multi_target.clone(), vec![])),
        ("python-single-r1".to_string(), (single_target, vec![])),
        ("python-any-r1".to_string(), (multi_target, vec![Key::BDEPEND, Key::DEPEND])),
    ]
    .into_iter()
    .collect();

    let possible_use = [UseDepKind::Enabled, UseDepKind::Equal, UseDepKind::EnabledConditional]
        .into_iter()
        .collect();

    Check {
        repo,
        eclasses,
        possible_use,
        params,
    }
}

struct Check {
    repo: &'static Repo,
    eclasses: IndexSet<&'static Eclass>,
    possible_use: HashSet<UseDepKind>,
    params: HashMap<String, (IndexSet<&'static String>, Vec<Key>)>,
}

super::register!(Check);

impl VersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        // TODO: return on multiple matches
        let Some(eclass) = pkg.inherited().intersection(&self.eclasses).next() else {
            return;
        };

        let Some((available_targets, keys)) = self.params.get(eclass.name()) else {
            return;
        };
        let deps: IndexSet<_> = pkg.dependencies(keys).into_iter_flatten().collect();

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
        let Some(idx) = available_targets.get_index_of(&latest_target) else {
            return;
        };

        let mut targets = available_targets.as_slice()[idx + 1..]
            .iter()
            .map(|x| x.as_str())
            .collect::<Vec<_>>();

        if targets.is_empty() {
            return;
        }

        let prefixed = if eclass.name() == "python-r1" {
            |s: &str| -> bool { s.starts_with("python_targets_") }
        } else {
            |s: &str| -> bool {
                s.starts_with("python_targets_") || s.starts_with("python_single_target_")
            }
        };

        for (dep, use_deps) in deps.iter().filter_map(|x| x.use_deps().map(|u| (x, u))) {
            if use_deps
                .iter()
                .any(|x| self.possible_use.contains(x.kind()) && prefixed(x.flag()))
            {
                if let Some(pkg) = self.repo.iter_restrict(dep.no_use_deps().as_ref()).last() {
                    let iuse = pkg
                        .iuse()
                        .iter()
                        .filter(|x| prefixed(x.flag()))
                        .map(|x| format!("python{}", x.flag().rsplit_once("python").unwrap().1))
                        .collect::<HashSet<_>>();
                    targets.retain(|x| iuse.contains(*x));
                    if targets.is_empty() {
                        return;
                    }
                }
            }
        }

        let message = targets.iter().join(", ");
        filter.report(PythonUpdate.version(pkg, message));
    }
}
