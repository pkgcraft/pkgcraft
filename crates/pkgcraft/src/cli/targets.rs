use std::collections::HashSet;
use std::ops::Deref;

use camino::Utf8Path;
use itertools::Itertools;
use strum::IntoEnumIterator;

use crate::config::Config;
use crate::dep::Cpn;
use crate::pkg::ebuild::{EbuildPkg, EbuildRawPkg};
use crate::pkg::{Package, Pkg, RepoPackage};
use crate::repo::ebuild::EbuildRepo;
use crate::repo::set::RepoSet;
use crate::repo::{PkgRepository, Repo, RepoFormat, Repository};
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::{self, Restrict, Scope};
use crate::traits::Contains;
use crate::types::OrderedMap;
use crate::utils::current_dir;
use crate::Error;

/// Convert a target ebuild repo arg into an ebuild repo reference.
pub fn target_ebuild_repo(config: &mut Config, target: &str) -> crate::Result<EbuildRepo> {
    // load system config for repo alias support
    if !target.contains(std::path::MAIN_SEPARATOR) {
        config.load()?;
    }

    let id = if config.repos.get(target).is_some() {
        target.to_string()
    } else if let Ok(abspath) = Utf8Path::new(target).canonicalize_utf8() {
        config.add_repo_path(&abspath, &abspath, 0, true)?;
        abspath.to_string()
    } else {
        return Err(Error::InvalidValue(format!("unknown repo: {target}")));
    };

    config
        .repos
        .get(&id)
        .and_then(|r| r.as_ebuild())
        .cloned()
        .ok_or_else(|| Error::InvalidValue(format!("non-ebuild repo: {target}")))
}

pub struct TargetRestrictions<'a> {
    config: &'a mut Config,
    repo_set: RepoSet,
    repo_format: Option<RepoFormat>,
    scopes: Option<HashSet<Scope>>,
}

impl<'a> TargetRestrictions<'a> {
    /// Create a target restrictions parser.
    pub fn new(config: &'a mut Config) -> Self {
        Self {
            config,
            repo_set: Default::default(),
            repo_format: Default::default(),
            scopes: Default::default(),
        }
    }

    /// Set the allowed repo format.
    pub fn repo_format(mut self, value: RepoFormat) -> Self {
        self.repo_format = Some(value);
        self
    }

    /// Set the allowed restriction scopes via a matching filter function.
    pub fn scope<F>(mut self, f: F) -> Self
    where
        F: Fn(&Scope) -> bool,
    {
        self.scopes = Some(Scope::iter().filter(f).collect());
        self
    }

    /// Use a specific repo for target restrictions.
    ///
    /// This can either be the repo's configured name or the path to an external repo.
    ///
    /// When None is passed, the current working directory is tried.
    pub fn repo(mut self, value: Option<&str>) -> crate::Result<Self> {
        if let Some(s) = value.as_ref() {
            // load system config for repo alias support
            if !s.contains(std::path::MAIN_SEPARATOR) {
                self.config.load()?;
            }

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

    fn repo_set(&mut self) -> crate::Result<&RepoSet> {
        if self.repo_set.repos.is_empty() {
            self.config.load()?;
            self.repo_set = self.config.repos.set(self.repo_format);
        }
        Ok(&self.repo_set)
    }

    fn repo_from_path<P: AsRef<Utf8Path>>(&mut self, path: P) -> crate::Result<Repo> {
        let path = path.as_ref();
        if let Some(format) = self.repo_format {
            self.config
                .add_format_repo_path(path, path, 0, true, format)
        } else {
            self.config.add_repo_path(path, path, 0, true)
        }
    }

    fn repo_from_nested_path<P: AsRef<Utf8Path>>(&mut self, path: P) -> crate::Result<Repo> {
        if let Some(format) = self.repo_format {
            self.config.add_format_repo_nested_path(path, 0, format)
        } else {
            self.config.add_nested_repo_path(path, 0)
        }
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
                    let path = Utf8Path::new(path).canonicalize_utf8().map_err(|e| {
                        Error::InvalidValue(format!("invalid repo: {path}: {e}"))
                    })?;

                    // add external repo to the config if it doesn't exist
                    let repo = if let Some(repo) =
                        self.repo_set()?.repos.iter().find(|r| r.path() == path)
                    {
                        repo.clone()
                    } else {
                        self.repo_from_path(&path)?
                    };

                    return Ok((repo.into(), Restrict::and(restricts)));
                }
                [id] if !self.repo_set()?.repos.iter().any(|r| r.id() == id) => {
                    return Err(Error::InvalidValue(format!("unknown repo: {id}")));
                }
                _ => (),
            }
        }

        Ok(self.repo_set()?.clone().filter(restrict))
    }

    /// Convert a target into a path or dep restriction.
    fn target_restriction(&mut self, target: &str) -> crate::Result<(RepoSet, Restrict)> {
        let path_target = Utf8Path::new(target)
            .canonicalize_utf8()
            .map_err(|e| Error::InvalidValue(format!("invalid path target: {target}: {e}")));
        let repo_target = path_target
            .as_ref()
            .ok()
            .map(|_| self.repo_from_nested_path(target));

        // avoid treating `cat/pkg/` as path restriction
        let s = target.trim_end_matches('/');

        match (restrict::parse::dep(s), path_target, repo_target) {
            // prefer dep restrictions for valid cat/pkg paths
            (Ok(restrict), Ok(_), _) if s.contains('/') => self.dep_restriction(restrict),
            (_, Ok(path), Some(Ok(repo))) => repo
                .restrict_from_path(&path)
                .ok_or_else(|| {
                    Error::InvalidValue(format!("{repo} doesn't contain path: {path}"))
                })
                .map(|restrict| (repo.into(), restrict)),
            (Ok(restrict), _, _) => self.dep_restriction(restrict),
            (_, Ok(path), Some(Err(e))) if path.exists() => Err(e),
            (_, Err(e), _) if s.contains('/') => Err(e),
            (Err(e), _, _) => Err(e),
        }
        // verify restriction matches required scopes, if any exist
        .and_then(|(set, restrict)| {
            if let Some(values) = self.scopes.as_ref() {
                let scope = Scope::from(&restrict);
                if !values.contains(&scope) {
                    return Err(Error::InvalidValue(format!(
                        "invalid {scope} scope: {target}"
                    )));
                }
            }
            Ok((set, restrict))
        })
    }

    /// Determine target restrictions.
    pub fn targets<I>(
        mut self,
        values: I,
    ) -> impl Iterator<Item = crate::Result<(RepoSet, Restrict)>> + 'a
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
        <I as IntoIterator>::IntoIter: 'a,
    {
        values
            .into_iter()
            .map(move |s| self.target_restriction(s.as_ref()))
    }

    /// Determine target restrictions and finalize the config.
    pub fn finalize_targets<I>(mut self, values: I) -> crate::Result<Vec<(RepoSet, Restrict)>>
    where
        I: IntoIterator,
        I::Item: std::fmt::Display,
    {
        // convert targets into restrictions, initializing repos as necessary
        let mut targets = vec![];
        for target in values {
            let target = target.to_string();
            let (set, restrict) = self.target_restriction(&target)?;
            targets.push((target, set, restrict));
        }

        // finalize the config after loading repos to start the build pool
        self.config.finalize()?;

        // verify matches exist
        targets
            .into_iter()
            .map(|(target, set, restrict)| {
                if set.contains(&restrict) {
                    Ok((set, restrict))
                } else {
                    Err(Error::NoMatches(target))
                }
            })
            .try_collect()
    }
}

/// Convert target restrictions to packages.
pub fn pkgs<I>(values: I) -> impl Iterator<Item = crate::Result<Pkg>>
where
    I: IntoIterator<Item = (RepoSet, Restrict)>,
{
    values
        .into_iter()
        .flat_map(|(set, restrict)| set.iter_restrict(restrict))
}

/// Convert target restrictions to ebuild packages.
pub fn ebuild_pkgs<I>(values: I) -> impl Iterator<Item = crate::Result<EbuildPkg>>
where
    I: IntoIterator<Item = (RepoSet, Restrict)>,
{
    values.into_iter().flat_map(|(set, restrict)| {
        set.into_iter()
            .filter_map(|r| r.into_ebuild().ok())
            .flat_map(move |r| r.iter_restrict_ordered(&restrict))
    })
}

/// Convert target restrictions into expanded ebuild package data.
///
/// This is useful to create pkg sets while still being able to log or ignore errors.
pub fn ebuild_pkgs_expand<I>(
    values: I,
) -> impl Iterator<Item = crate::Result<((EbuildRepo, Cpn), EbuildPkg)>>
where
    I: IntoIterator<Item = (RepoSet, Restrict)>,
{
    ebuild_pkgs(values).map(|result| result.map(|pkg| ((pkg.repo(), pkg.cpn().clone()), pkg)))
}

/// Convert target restrictions into ebuild package sets.
pub fn ebuild_pkg_sets<I>(
    values: I,
) -> crate::Result<OrderedMap<(EbuildRepo, Cpn), Vec<EbuildPkg>>>
where
    I: IntoIterator<Item = (RepoSet, Restrict)>,
{
    ebuild_pkgs_expand(values).try_collect()
}

/// Convert target restrictions to raw ebuild packages.
pub fn ebuild_raw_pkgs<I>(values: I) -> impl Iterator<Item = crate::Result<EbuildRawPkg>>
where
    I: IntoIterator<Item = (RepoSet, Restrict)>,
{
    values.into_iter().flat_map(|(set, restrict)| {
        set.into_iter()
            .filter_map(|r| r.into_ebuild().ok())
            .flat_map(move |r| r.iter_raw_restrict(&restrict))
    })
}
