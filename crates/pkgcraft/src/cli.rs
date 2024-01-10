use camino::Utf8Path;

use crate::config::Config;
use crate::repo::set::RepoSet;
use crate::repo::{RepoFormat, Repository};
use crate::restrict::{self, Restrict};
use crate::Error;

/// Convert a target into a path or dep restriction.
pub fn target_restriction(
    config: &mut Config,
    format: Option<RepoFormat>,
    target: &str,
) -> crate::Result<(RepoSet, Restrict)> {
    let path_target = Utf8Path::new(target).canonicalize_utf8();
    let repo_set = config.repos.set(format);
    let repo_format = format.unwrap_or_default();

    if let Ok(path) = &path_target {
        if path.exists() {
            if let Some((repo, restrict)) = repo_set
                .repos()
                .iter()
                .filter_map(|repo| repo.restrict_from_path(path).map(|r| (repo, r)))
                .next()
            {
                // target is an configured repo path restrict
                return Ok((RepoSet::from_iter([repo]), restrict));
            } else if let Ok(repo) = repo_format.load_from_nested_path(path, 0, path, true) {
                // target is an external repo path restrict
                config.add_repo(&repo, true)?;
                let restrict = repo.restrict_from_path(path).expect("invalid repo path");
                return Ok((RepoSet::from_iter([&repo]), restrict));
            } else {
                return Err(Error::InvalidValue(format!("invalid repo path: {path}")));
            }
        }
    }

    match (restrict::parse::dep(target), path_target) {
        (Ok(restrict), _) => Ok(repo_set.filter(restrict)),
        (_, Err(e)) if target.starts_with(['.', '/']) => {
            Err(Error::InvalidValue(format!("invalid path target: {target}: {e}")))
        }
        (Err(e), _) => Err(e),
    }
}
