use pkgcraft::dep::Dep;

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
