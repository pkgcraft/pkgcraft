use camino::Utf8Path;
use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::Repo as EbuildRepo;

/// Convert a target ebuild repo arg into an ebuild repo reference.
pub(crate) fn target_ebuild_repo<'a>(
    config: &'a mut Config,
    target: &str,
) -> anyhow::Result<&'a EbuildRepo> {
    let id = if config.repos.get(target).is_some() {
        target.to_string()
    } else if let Ok(abspath) = Utf8Path::new(target).canonicalize_utf8() {
        config.add_repo_path(&abspath, &abspath, 0, true)?;
        abspath.to_string()
    } else {
        anyhow::bail!("unknown repo: {target}");
    };

    if let Some(r) = config.repos.get(&id).and_then(|r| r.as_ebuild()) {
        Ok(r.as_ref())
    } else {
        anyhow::bail!("non-ebuild repo: {target}")
    }
}
