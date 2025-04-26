use pkgcraft::dep::Dep;
use pkgcraft::repo::EbuildRepo;

/// Return true if a given package dependency has a USE dependency starting with at least
/// one of the given prefixes, otherwise false.
pub(crate) fn use_starts_with<S: AsRef<str>>(dep: &Dep, prefixes: &[S]) -> bool {
    dep.use_deps()
        .map(|u| {
            u.iter().any(|x| {
                x.enabled() && prefixes.iter().any(|s| x.flag().starts_with(s.as_ref()))
            })
        })
        .unwrap_or_default()
}

/// Pull USE_EXPAND targets related to a given name from a target repo.
pub(crate) fn use_expand(
    repo: &EbuildRepo,
    name: &str,
    prefix: &str,
) -> impl Iterator<Item = String> {
    repo.use_expand()
        .get(name)
        .map(|x| {
            x.keys()
                .filter(|x| x.starts_with(prefix))
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
}

/// Return an iterator of matching implementation USE_EXPAND targets.
///
/// Assumes experimental targets don't end with ASCII digits and skips them, for example
/// the free threading targets for python.
pub(crate) fn impl_targets(
    repo: &EbuildRepo,
    name: &str,
    prefix: &str,
) -> impl Iterator<Item = String> {
    use_expand(repo, name, prefix).filter(|x| x.ends_with(|c: char| c.is_ascii_digit()))
}
