use std::ops::Deref;

use camino::Utf8Path;
use itertools::Itertools;

use crate::config::Config;
use crate::pkg;
use crate::repo::set::RepoSet;
use crate::repo::{PkgRepository, Repo, RepoFormat, Repository};
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::{self, Restrict};
use crate::types::IndexMap;
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
            let path = Utf8Path::new(s);
            let repo = if let Some(repo) = self.config.repos.get(s) {
                Ok(repo.clone())
            } else if path.exists() {
                self.repo_from_path(path)
            } else {
                Err(Error::InvalidValue(format!("unknown repo: {s}")))
            }?;
            self.repo_set = repo.into();
        } else if let Ok(repo) = current_dir().and_then(|x| self.repo_from_nested_path(&x)) {
            self.repo_set = repo.into();
        }

        Ok(self)
    }

    fn repo_from_path(&mut self, path: &Utf8Path) -> crate::Result<Repo> {
        self.config
            .add_format_repo_path(path, path, 0, true, self.repo_format)
    }

    fn repo_from_nested_path(&mut self, path: &Utf8Path) -> crate::Result<Repo> {
        self.config
            .add_format_repo_nested_path(path, 0, self.repo_format)
    }

    fn dep_restriction(&mut self, restrict: Restrict) -> crate::Result<(RepoSet, Restrict)> {
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
                    let path = Utf8Path::new(path)
                        .canonicalize_utf8()
                        .map_err(|e| Error::InvalidValue(format!("invalid repo: {path}: {e}")))?;

                    // add external repo to the config if it doesn't exist
                    let repo =
                        if let Some(repo) = self.repo_set.repos.iter().find(|r| r.path() == path) {
                            repo.clone()
                        } else {
                            self.repo_from_path(&path)?
                        };

                    return Ok((repo.into(), Restrict::and(restricts)));
                }
                [id] if !self.repo_set.repos.iter().any(|r| r.id() == id) => {
                    return Err(Error::InvalidValue(format!("unknown repo: {id}")));
                }
                _ => (),
            }
        }

        Ok(self.repo_set.clone().filter(restrict))
    }

    /// Convert a target into a path or dep restriction.
    fn target_restriction(&mut self, target: &str) -> crate::Result<(RepoSet, Restrict)> {
        let path_target = Utf8Path::new(target)
            .canonicalize_utf8()
            .map_err(|e| Error::InvalidValue(format!("invalid path target: {target}: {e}")));
        let repo_target = path_target
            .as_ref()
            .ok()
            .map(|path| self.repo_from_nested_path(path));

        match (restrict::parse::dep(target), path_target, repo_target) {
            // prefer dep restrictions for valid cat/pkg paths
            (Ok(restrict), Ok(_), _) if target.contains('/') => self.dep_restriction(restrict),
            (_, Ok(path), Some(Ok(repo))) => {
                let restrict = repo.restrict_from_path(path).expect("invalid repo");
                Ok((repo.into(), restrict))
            }
            (Ok(restrict), _, _) => self.dep_restriction(restrict),
            (_, Ok(path), Some(Err(e))) if path.exists() => Err(e),
            (_, Err(e), _) if target.contains('/') => Err(e),
            (Err(e), _, _) => Err(e),
        }
    }

    /// Determine target restrictions.
    pub fn targets<I>(mut self, values: I) -> crate::Result<IndexMap<RepoSet, Vec<Restrict>>>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        values
            .into_iter()
            .map(|s| self.target_restriction(s.as_ref()))
            .try_collect()
    }

    /// Determine target packages.
    pub fn pkgs<I>(mut self, values: I) -> impl Iterator<Item = crate::Result<pkg::Pkg>> + 'a
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
        <I as IntoIterator>::IntoIter: 'a,
    {
        values
            .into_iter()
            .map(move |s| self.target_restriction(s.as_ref()))
            .flat_map(move |result| {
                let iter: Box<dyn Iterator<Item = crate::Result<pkg::Pkg>>> = match result {
                    Ok((set, restrict)) => Box::new(set.iter_restrict(restrict).map(Ok)),
                    Err(e) => Box::new([Err(e)].into_iter()),
                };
                iter
            })
    }

    /// Determine target ebuild packages.
    pub fn pkgs_ebuild<I>(
        self,
        values: I,
    ) -> impl Iterator<Item = crate::Result<pkg::ebuild::Pkg>> + 'a
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
        <I as IntoIterator>::IntoIter: 'a,
    {
        self.pkgs(values).map(|r| match r {
            Ok(pkg::Pkg::Ebuild(pkg)) => Ok(pkg),
            Ok(pkg) => Err(Error::InvalidValue(format!("non-ebuild pkg: {pkg}"))),
            Err(e) => Err(e),
        })
    }

    /// Determine target raw ebuild packages.
    pub fn pkgs_ebuild_raw<I>(
        mut self,
        values: I,
    ) -> impl Iterator<Item = crate::Result<pkg::ebuild::raw::Pkg>> + 'a
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
        <I as IntoIterator>::IntoIter: 'a,
    {
        values
            .into_iter()
            .map(move |s| self.target_restriction(s.as_ref()))
            .flat_map(move |result| {
                let iter: Box<dyn Iterator<Item = crate::Result<pkg::ebuild::raw::Pkg>>> =
                    match result {
                        Ok((set, restrict)) => Box::new(
                            set.into_iter()
                                .filter_map(|r| r.into_ebuild().ok())
                                .flat_map(move |r| r.iter_raw_restrict(&restrict).map(Ok)),
                        ),
                        Err(e) => Box::new([Err(e)].into_iter()),
                    };
                iter
            })
    }
}
