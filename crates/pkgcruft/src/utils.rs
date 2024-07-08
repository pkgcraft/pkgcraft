use indexmap::IndexSet;
use pkgcraft::dep::Dep;
use pkgcraft::repo::ebuild::Repo;

/// Return true if a given package dependency has a USE dependency starting with at least
/// one of the given prefixes, otherwise false.
pub(crate) fn use_starts_with(dep: &Dep, prefixes: &[&str]) -> bool {
    dep.use_deps()
        .map(|u| {
            u.iter()
                .any(|x| x.enabled() && prefixes.iter().any(|s| x.flag().starts_with(*s)))
        })
        .unwrap_or_default()
}

/// Pull USE_EXPAND targets related to a given name from a target repo.
pub(crate) fn use_expand<'a>(repo: &'a Repo, name: &str, prefix: &str) -> IndexSet<&'a str> {
    repo.use_expand()
        .get(name)
        .map(|x| {
            x.keys()
                .filter(|x| x.starts_with(prefix))
                .map(|x| x.as_str())
                .collect()
        })
        .unwrap_or_default()
}
