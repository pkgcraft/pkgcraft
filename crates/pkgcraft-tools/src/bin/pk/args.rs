use camino::Utf8Path;
use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::EbuildRepo;

/// Convert a target ebuild repo arg into an ebuild repo reference.
pub(crate) fn target_ebuild_repo(config: &mut Config, target: &str) -> anyhow::Result<EbuildRepo> {
    let id = if config.repos.get(target).is_some() {
        target.to_string()
    } else if let Ok(abspath) = Utf8Path::new(target).canonicalize_utf8() {
        config.add_repo_path(&abspath, &abspath, 0, true)?;
        abspath.to_string()
    } else {
        anyhow::bail!("unknown repo: {target}");
    };

    config
        .repos
        .get(&id)
        .and_then(|r| r.as_ebuild())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("non-ebuild repo: {target}"))
}
