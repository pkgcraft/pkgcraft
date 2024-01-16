use std::ops::Deref;

use camino::Utf8Path;
use indexmap::IndexMap;

use crate::config::Config;
use crate::repo::set::RepoSet;
use crate::repo::{RepoFormat, Repository};
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::{self, Restrict};
use crate::utils::current_dir;
use crate::Error;

pub struct TargetRestrictions<'a> {
    config: &'a mut Config,
    repo_set: RepoSet,
    repo_format: RepoFormat,
}

impl<'a> TargetRestrictions<'a> {
    pub fn new(config: &'a mut Config) -> Self {
        let repo_set = config.repos.set(None);
        Self {
            config,
            repo_set,
            repo_format: Default::default(),
        }
    }

    pub fn repo_format(mut self, value: RepoFormat) -> Self {
        self.repo_format = value;
        self.repo_set = self.config.repos.set(Some(value));
        self
    }

    pub fn repo(mut self, value: Option<String>) -> crate::Result<Self> {
        if let Some(s) = value.as_ref() {
            if s.contains('/') && Utf8Path::new(s).exists() {
                let repo = self.repo_format.load_from_path(s, 0, s, true)?;
                self.config.add_repo(&repo, true)?;
                self.repo_set = RepoSet::from_iter([repo]);
            } else {
                let repo = self
                    .config
                    .repos
                    .get(s)
                    .ok_or_else(|| Error::InvalidValue(format!("unknown repo: {s}")))?;
                self.repo_set = RepoSet::from_iter([repo]);
            }
        } else if let Ok(path) = current_dir() {
            if let Ok(repo) = self
                .repo_format
                .load_from_nested_path(&path, 0, &path, true)
            {
                self.config.add_repo(&repo, true)?;
                self.repo_set = RepoSet::from_iter([repo]);
            }
        }

        Ok(self)
    }

    /// Convert a target into a path or dep restriction.
    fn target_restriction(&mut self, target: &str) -> crate::Result<(RepoSet, Restrict)> {
        let path_target = Utf8Path::new(target).canonicalize_utf8();
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

                    match paths[..] {
                        [path] if path.contains('/') => {
                            let path = Utf8Path::new(path).canonicalize_utf8().map_err(|e| {
                                Error::InvalidValue(format!("invalid repo path: {path}: {e}"))
                            })?;

                            // add external repo to the config if it doesn't exist
                            let repo = if let Some(repo) =
                                self.repo_set.repos().iter().find(|r| r.path() == path)
                            {
                                repo.clone()
                            } else {
                                let repo =
                                    self.repo_format.load_from_path(&path, 0, &path, true)?;
                                self.config.add_repo(&repo, true)?;
                                repo
                            };

                            return Ok((RepoSet::from_iter([&repo]), Restrict::and(restricts)));
                        }
                        [id] if !self.repo_set.repos().iter().any(|r| r.id() == id) => {
                            return Err(Error::InvalidValue(format!("unknown repo: {id}")));
                        }
                        _ => (),
                    }
                }

                Ok(self.repo_set.clone().filter(restrict))
            }
            (_, Ok(path)) if path.exists() => {
                if let Some((repo, restrict)) = self
                    .repo_set
                    .repos()
                    .iter()
                    .find_map(|repo| repo.restrict_from_path(&path).map(|r| (repo, r)))
                {
                    // configured repo path restrict
                    Ok((RepoSet::from_iter([repo]), restrict))
                } else if let Ok(repo) = self
                    .repo_format
                    .load_from_nested_path(&path, 0, &path, true)
                {
                    // external repo path restrict
                    self.config.add_repo(&repo, true)?;
                    let restrict = repo.restrict_from_path(&path).expect("invalid repo path");
                    Ok((RepoSet::from_iter([&repo]), restrict))
                } else {
                    Err(Error::InvalidValue(format!("invalid repo path: {path}")))
                }
            }
            (_, Err(e)) if target.contains('/') => {
                Err(Error::InvalidValue(format!("invalid path target: {target}: {e}")))
            }
            (Err(e), _) => Err(e),
        }
    }

    pub fn targets<I, S>(
        mut self,
        values: I,
    ) -> crate::Result<impl Iterator<Item = (RepoSet, Vec<Restrict>)>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        // determine target restrictions
        let targets: Result<Vec<_>, _> = values
            .into_iter()
            .map(|s| self.target_restriction(s.as_ref()))
            .collect();
        let targets = targets?;

        // TODO: Implement custom types for ordered maps of ordered collections so FromIterator
        // works directly instead of instead of having to first collect to a vector.
        let mut collapsed_targets = IndexMap::<_, Vec<_>>::new();
        for (set, restrict) in targets {
            collapsed_targets.entry(set).or_default().push(restrict);
        }

        Ok(collapsed_targets.into_iter())
    }
}

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
                // configured repo path restrict
                return Ok((RepoSet::from_iter([repo]), restrict));
            } else if let Ok(repo) = repo_format.load_from_nested_path(path, 0, path, true) {
                // external repo path restrict
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

                match paths[..] {
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
                    [id] if !repo_set.repos().iter().any(|r| r.id() == id) => {
                        return Err(Error::InvalidValue(format!("unknown repo: {id}")));
                    }
                    _ => (),
                }
            }

            Ok(repo_set.filter(restrict))
        }
        (_, Err(e)) if target.contains('/') => {
            Err(Error::InvalidValue(format!("invalid path target: {target}: {e}")))
        }
        (Err(e), _) => Err(e),
    }
}
