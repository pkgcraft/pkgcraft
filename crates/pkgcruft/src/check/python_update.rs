use std::collections::HashSet;

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use once_cell::sync::Lazy;
use pkgcraft::dep::Flatten;
use pkgcraft::pkg::ebuild::metadata::Key::{self, BDEPEND, DEPEND};
use pkgcraft::pkg::ebuild::Pkg;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::PkgRepository;

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

// TODO: add inherited use_expand support to pkgcraft so running against overlays works
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
    let params = ECLASSES
        .iter()
        .copied()
        .map(|x| match x {
            "python-r1" => (x, (use_expand(repo, "python_targets"), vec![], vec![IUSE_PREFIX])),
            "python-single-r1" => (
                x,
                (
                    use_expand(repo, "python_single_target"),
                    vec![],
                    vec![IUSE_PREFIX, IUSE_PREFIX_S],
                ),
            ),
            "python-any-r1" => (
                x,
                (
                    use_expand(repo, "python_targets"),
                    vec![BDEPEND, DEPEND],
                    vec![IUSE_PREFIX, IUSE_PREFIX_S],
                ),
            ),
            _ => unreachable!("{CHECK}: unsupported eclass: {x}"),
        })
        .collect();

    Check { repo, params }
}

// parameters used for scanning deps related to python eclasses
type Params = (Vec<&'static str>, Vec<Key>, Vec<&'static str>);

struct Check {
    repo: &'static Repo,
    params: IndexMap<&'static str, Params>,
}

super::register!(Check);

impl VersionCheck for Check {
    fn run(&self, pkg: &Pkg, filter: &mut ReportFilter) {
        let Some((available_targets, keys, prefixes)) = self
            .params
            .iter()
            .find_map(|(k, v)| pkg.inherited().get(*k).and(Some(v)))
        else {
            return;
        };

        let deps: IndexSet<_> = pkg
            .dependencies(keys)
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
        let mut targets = available_targets
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
            .filter(|x| use_starts_with(x, prefixes))
            .filter_map(|x| self.repo.iter_restrict(x.no_use_deps()).last())
        {
            let iuse = pkg
                .iuse()
                .iter()
                .filter_map(|x| deprefix(x.flag(), prefixes))
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
