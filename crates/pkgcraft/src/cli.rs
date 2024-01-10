use std::ops::Deref;

use camino::Utf8Path;

use crate::config::Config;
use crate::repo::set::RepoSet;
use crate::repo::{RepoFormat, Repository};
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
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
                .find_map(|repo| repo.restrict_from_path(path).map(|r| (repo, r)))
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
        (Ok(restrict), _) => {
            // support external repo path restrictions
            if let Restrict::And(vals) = &restrict {
                use DepRestrict::Repo;
                use StrRestrict::Equal;
                let mut paths = vec![];
                let mut restricts = vec![];
                for r in vals.iter().map(Deref::deref) {
                    match r {
                        Restrict::Dep(Repo(Some(Equal(s)))) => paths.push(s),
                        r => restricts.push(r),
                    }
                }

                match &paths[..] {
                    [path] if path.contains('/') => {
                        let path = Utf8Path::new(path).canonicalize_utf8().map_err(|e| {
                            Error::InvalidValue(format!("invalid repo path: {path}: {e}"))
                        })?;

                        // add external repo to the config if it doesn't exist
                        let repo = if let Some(repo) =
                            repo_set.repos().iter().find(|r| r.path() == path)
                        {
                            repo.clone()
                        } else {
                            let repo = repo_format.load_from_path(&path, 0, &path, true)?;
                            config.add_repo(&repo, true)?;
                            repo
                        };

                        return Ok((RepoSet::from_iter([&repo]), Restrict::and(restricts)));
                    }
                    _ => (),
                }
            }

            Ok(repo_set.filter(restrict))
        }
        (_, Err(e)) if target.starts_with(['.', '/']) => {
            Err(Error::InvalidValue(format!("invalid path target: {target}: {e}")))
        }
        (Err(e), _) => Err(e),
    }
}
