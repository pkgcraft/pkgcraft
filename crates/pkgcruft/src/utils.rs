use pkgcraft::dep::Dep;
use pkgcraft::repo::ebuild::Repo;

/// Return true if a given package dependency has a USE dependency starting with at least
/// one of the given prefixes, otherwise false.
pub(crate) fn use_starts_with(dep: &Dep, prefixes: &[&str]) -> bool {
    dep.use_deps()
        .map(|u| {
            u.iter()
                .any(|x| x.possible() && prefixes.iter().any(|s| x.flag().starts_with(*s)))
        })
        .unwrap_or_default()
}

// TODO: add inherited use_expand support to pkgcraft so running against overlays works
/// Pull USE_EXPAND targets related to a given name from a target repo.
pub(crate) fn use_expand<'a>(repo: &'a Repo, name: &str, prefix: &str) -> Vec<&'a str> {
    repo.metadata
        .use_expand()
        .get(name)
        .map(|x| {
            x.keys()
                .filter(|x| x.starts_with(prefix))
                .map(|x| x.as_str())
                .collect()
        })
        .unwrap_or_default()
}
