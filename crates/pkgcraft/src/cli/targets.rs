use std::collections::HashSet;
use std::ops::Deref;

use camino::Utf8Path;
use indexmap::{IndexMap, IndexSet};
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

    /// Return the target repo set.
    ///
    /// Note that the system config is loaded if no repos are currently targeted.
    fn repo_set(&mut self) -> crate::Result<&RepoSet> {
        if self.repo_set.repos.is_empty() {
            self.config.load()?;
            self.repo_set = self.config.repos.set(self.repo_format);
        }
        Ok(&self.repo_set)
    }

    /// Load a repo from a path.
    fn repo_from_path<P: AsRef<Utf8Path>>(&mut self, path: P) -> crate::Result<Repo> {
        let path = path.as_ref();
        if let Some(format) = self.repo_format {
            self.config
                .add_format_repo_path(path, path, 0, true, format)
        } else {
            self.config.add_repo_path(path, path, 0, true)
        }
    }

    /// Load a repo from a nested path.
    fn repo_from_nested_path<P: AsRef<Utf8Path>>(&mut self, path: P) -> crate::Result<Repo> {
        if let Some(format) = self.repo_format {
            self.config.add_format_repo_nested_path(path, 0, format)
        } else {
            self.config.add_nested_repo_path(path, 0)
        }
    }

    /// Parse a dep restriction.
    fn dep_restriction(&mut self, restrict: Restrict) -> crate::Result<(RepoSet, Restrict)> {
        let repo_set = self.repo_set()?;

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
                    let repo =
                        if let Some(repo) = repo_set.repos.iter().find(|r| r.path() == path) {
                            repo.clone()
                        } else {
                            self.repo_from_path(&path)?
                        };

                    return Ok((repo.into(), Restrict::and(restricts)));
                }
                [id] if !repo_set.repos.iter().any(|r| r.id() == id) => {
                    return Err(Error::InvalidValue(format!("unknown repo: {id}")));
                }
                _ => (),
            }
        }

        Ok(repo_set.clone().filter(restrict))
    }

    /// Convert a target into a path or dep restriction.
    fn target_restriction(&mut self, target: &str) -> crate::Result<(RepoSet, Restrict)> {
        // avoid treating `cat/pkg/` as path restriction
        let mut s = target.trim_end_matches('/');

        let (set, restrict) = if let Ok(cpn) = Cpn::try_new(s) {
            Ok((self.repo_set()?.clone(), cpn.into()))
        } else {
            // convert glob to current path restriction
            s = if s == "*" { "." } else { s };

            // try creating path and repo targets
            let path_target = Utf8Path::new(s).canonicalize_utf8().map_err(|e| {
                Error::InvalidValue(format!("invalid path target: {target}: {e}"))
            });
            let repo_target = path_target
                .as_ref()
                .ok()
                .map(|_| self.repo_from_nested_path(s));

            match (restrict::parse::dep(s), path_target, repo_target) {
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
        }?;

        // verify restriction matches required scopes, if any exist
        if let Some(values) = self.scopes.as_ref() {
            let scope = Scope::from(&restrict);
            if !values.contains(&scope) {
                return Err(Error::InvalidValue(format!("invalid {scope} scope: {target}")));
            }
        }

        Ok((set, restrict))
    }

    /// Determine target restrictions and finalize the config.
    pub fn finalize_targets<I>(mut self, values: I) -> crate::Result<Targets>
    where
        I: IntoIterator,
        I::Item: std::fmt::Display,
    {
        // convert targets into restrictions, initializing repos as necessary
        let mut targets = IndexMap::<_, Vec<_>>::new();
        for target in values {
            let target = target.to_string();
            let (set, restrict) = self.target_restriction(&target)?;
            targets.entry(set).or_default().push((target, restrict));
        }

        // finalize the config after loading repos to start the build pool
        self.config.finalize()?;

        // verify matches exist and collapse targets
        let mut collapsed_targets = vec![];
        for (set, values) in targets {
            let restricts: IndexSet<_> = values
                .into_iter()
                .map(|(target, restrict)| {
                    if set.contains(&restrict) {
                        Ok(restrict)
                    } else {
                        Err(Error::NoMatches(target))
                    }
                })
                .try_collect()?;
            collapsed_targets.push((set, Restrict::or(restricts)));
        }

        Ok(Targets(collapsed_targets))
    }
}

pub struct Targets(Vec<(RepoSet, Restrict)>);

impl IntoIterator for Targets {
    type Item = (RepoSet, Restrict);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Targets {
    /// Convert target restrictions to packages.
    pub fn pkgs(self) -> impl Iterator<Item = crate::Result<Pkg>> {
        self.into_iter()
            .flat_map(|(set, restrict)| set.iter_restrict(restrict))
    }

    /// Convert target restrictions to ebuild packages.
    pub fn ebuild_pkgs(self) -> impl Iterator<Item = crate::Result<EbuildPkg>> {
        self.into_iter().flat_map(|(set, restrict)| {
            set.into_iter()
                .filter_map(|r| r.into_ebuild().ok())
                .flat_map(move |r| r.iter_restrict_ordered(&restrict))
        })
    }

    /// Convert target restrictions into expanded ebuild package data.
    ///
    /// This is useful to create pkg sets while still being able to log or ignore errors.
    pub fn ebuild_pkgs_expand(
        self,
    ) -> impl Iterator<Item = crate::Result<((EbuildRepo, Cpn), EbuildPkg)>> {
        self.ebuild_pkgs()
            .map(|result| result.map(|pkg| ((pkg.repo(), pkg.cpn().clone()), pkg)))
    }

    /// Convert target restrictions into ebuild package sets.
    pub fn ebuild_pkg_sets(
        self,
    ) -> crate::Result<OrderedMap<(EbuildRepo, Cpn), Vec<EbuildPkg>>> {
        self.ebuild_pkgs_expand().try_collect()
    }

    /// Convert target restrictions to raw ebuild packages.
    pub fn ebuild_raw_pkgs(self) -> impl Iterator<Item = crate::Result<EbuildRawPkg>> {
        self.into_iter().flat_map(|(set, restrict)| {
            set.into_iter()
                .filter_map(|r| r.into_ebuild().ok())
                .flat_map(move |r| r.iter_raw_restrict(&restrict))
        })
    }
}
