use std::collections::HashSet;
use std::fs;

use camino::Utf8Path;
use indexmap::IndexSet;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use scallop::pool::PoolSendIter;
use strum::{Display, EnumString};
use tracing::error;
use walkdir::WalkDir;

use crate::dep::Cpv;
use crate::error::{Error, PackageError};
use crate::files::{is_file, is_hidden};
use crate::pkg::ebuild::raw::Pkg;
use crate::repo::PkgRepository;
use crate::shell::metadata::Metadata;
use crate::utils::bounded_jobs;

use super::Repo;

pub(crate) mod md5_dict;

pub trait CacheEntry {
    /// Deserialize a cache entry to package metadata.
    fn deserialize<'a>(&self, pkg: &Pkg<'a>) -> crate::Result<Metadata<'a>>;
    /// Verify a cache entry is valid.
    fn verify(&self, pkg: &Pkg) -> crate::Result<()>;
}

pub trait Cache {
    type Entry: CacheEntry;
    fn format(&self) -> CacheFormat;
    fn path(&self) -> &Utf8Path;
    /// Get the cache entry for a given package.
    fn get(&self, pkg: &Pkg) -> crate::Result<Self::Entry>;
    /// Update the cache with the given package metadata.
    fn update(&self, pkg: &Pkg, meta: &Metadata) -> crate::Result<()>;
}

#[derive(
    Display, EnumString, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
#[strum(serialize_all = "kebab-case")]
pub enum CacheFormat {
    #[default]
    Md5Dict,
}

impl CacheFormat {
    /// Create a metadata cache using a given format at the default repo location.
    pub fn from_repo(&self, repo: &Repo) -> MetadataCache {
        match self {
            Self::Md5Dict => MetadataCache::Md5Dict(md5_dict::Md5Dict::from_repo(repo)),
        }
    }

    /// Create a metadata cache using a given format at a custom path.
    pub fn from_path<P: AsRef<Utf8Path>>(&self, path: P) -> MetadataCache {
        match self {
            Self::Md5Dict => MetadataCache::Md5Dict(md5_dict::Md5Dict::from_path(path)),
        }
    }
}

#[derive(Debug)]
pub enum MetadataCacheEntry {
    Md5Dict(md5_dict::Md5DictEntry),
}

impl CacheEntry for MetadataCacheEntry {
    fn deserialize<'a>(&self, pkg: &Pkg<'a>) -> crate::Result<Metadata<'a>> {
        match self {
            Self::Md5Dict(entry) => entry.deserialize(pkg),
        }
    }

    fn verify(&self, pkg: &Pkg) -> crate::Result<()> {
        match self {
            Self::Md5Dict(entry) => entry.verify(pkg),
        }
    }
}

#[derive(Debug)]
pub enum MetadataCache {
    Md5Dict(md5_dict::Md5Dict),
}

impl Cache for MetadataCache {
    type Entry = MetadataCacheEntry;

    fn format(&self) -> CacheFormat {
        match self {
            Self::Md5Dict(cache) => cache.format(),
        }
    }

    fn path(&self) -> &Utf8Path {
        match self {
            Self::Md5Dict(cache) => cache.path(),
        }
    }

    fn get(&self, pkg: &Pkg) -> crate::Result<Self::Entry> {
        match self {
            Self::Md5Dict(cache) => cache.get(pkg).map(MetadataCacheEntry::Md5Dict),
        }
    }

    /// Update the cache with the given package metadata.
    fn update(&self, pkg: &Pkg, meta: &Metadata) -> crate::Result<()> {
        match self {
            Self::Md5Dict(cache) => cache.update(pkg, meta),
        }
    }
}

impl MetadataCache {
    /// Create a regeneration builder for the cache.
    pub fn regen(&self) -> MetadataCacheRegen {
        MetadataCacheRegen {
            cache: self,
            jobs: num_cpus::get(),
            force: false,
            progress: false,
            suppress: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetadataCacheRegen<'a> {
    cache: &'a MetadataCache,
    jobs: usize,
    force: bool,
    progress: bool,
    suppress: bool,
}

impl MetadataCacheRegen<'_> {
    /// Set the number of parallel jobs to run.
    pub fn jobs(mut self, jobs: usize) -> Self {
        self.jobs = bounded_jobs(jobs);
        self
    }

    /// Force metadata regeneration across all packages.
    pub fn force(mut self, value: bool) -> Self {
        self.force = value;
        self
    }

    /// Show a progress bar during cache regeneration.
    pub fn progress(mut self, value: bool) -> Self {
        self.progress = value;
        self
    }

    /// Suppress output from stdout and stderr during cache regeneration.
    pub fn suppress(mut self, value: bool) -> Self {
        self.suppress = value;
        self
    }

    /// Regenerate the package metadata cache, returning the number of errors that occurred.
    pub fn run(&self, repo: &Repo) -> crate::Result<()> {
        // collapse lazy repo fields used during metadata generation
        repo.collapse_cache_regen();

        // initialize pool first to minimize forked process memory pages
        let func = |cpv: Cpv<String>| -> scallop::Result<()> {
            let pkg = Pkg::try_new(cpv, repo)?;
            let meta = Metadata::try_from(&pkg).map_err(|e| pkg.invalid_pkg_err(e))?;
            self.cache.update(&pkg, &meta)?;
            Ok(())
        };
        let pool = PoolSendIter::new(self.jobs, func, self.suppress)?;

        // use progress bar to show completion progress if enabled
        let pb = if self.progress {
            ProgressBar::new(0)
        } else {
            ProgressBar::hidden()
        };
        pb.set_style(ProgressStyle::with_template("{wide_bar} {msg} {pos}/{len}").unwrap());

        // TODO: replace with parallel Cpv iterator -- repo.par_iter_cpvs()
        // pull all package Cpvs from the repo
        let mut cpvs: IndexSet<_> = repo
            .categories()
            .into_par_iter()
            .flat_map(|s| repo.category_cpvs(&s))
            .collect();

        // set progression length encompassing all pkgs
        pb.set_length(cpvs.len().try_into().unwrap());

        if self.cache.path().exists() {
            // TODO: replace with parallelized cache iterator
            let entries: Vec<_> = WalkDir::new(self.cache.path())
                .min_depth(2)
                .max_depth(2)
                .into_iter()
                .collect();

            // remove outdated cache entries lacking matching ebuilds in parallel
            entries
                .into_par_iter()
                .filter_map(|e| e.ok())
                .filter(|e| is_file(e) && !is_hidden(e))
                .for_each(|e| {
                    let file = e.path();
                    let cpv_str = file
                        .strip_prefix(self.cache.path())
                        .expect("invalid metadata entry")
                        .to_string_lossy();
                    if let Ok(cpv) = Cpv::parse(&cpv_str) {
                        // Remove an outdated cache file and its potentially, empty parent
                        // directory while ignoring any I/O errors.
                        if !cpvs.contains(&cpv) {
                            fs::remove_file(file).ok();
                            fs::remove_dir(file.parent().unwrap()).ok();
                        }
                    }
                });

            if !self.force {
                // run cache validation in a thread pool
                pb.set_message("validating metadata cache:");
                cpvs = cpvs
                    .into_par_iter()
                    .filter(|cpv| {
                        pb.inc(1);
                        Pkg::try_new(cpv.clone(), repo)
                            .and_then(|pkg| self.cache.get(&pkg))
                            .is_err()
                    })
                    .collect();

                // reset progression in case validation decreased cpvs
                pb.set_position(0);
                pb.set_length(cpvs.len().try_into().unwrap());
            }
        }

        // send Cpvs and iterate over returned results, tracking progress and errors
        let mut errors = 0;
        if !cpvs.is_empty() {
            // create metadata directories in parallel
            let categories: HashSet<_> = cpvs.par_iter().map(|cpv| cpv.category()).collect();
            categories.into_par_iter().try_for_each(|cat| {
                let path = self.cache.path().join(cat);
                fs::create_dir_all(&path)
                    .map_err(|e| Error::IO(format!("failed creating metadata dir: {path}: {e}")))
            })?;

            pb.set_message("generating metadata cache:");
            for r in pool.iter(cpvs.into_iter())? {
                pb.inc(1);

                // log errors
                if let Err(e) = r {
                    errors += 1;
                    error!("{e}");
                }
            }
        }

        if errors > 0 {
            Err(Error::InvalidValue(
                "failed generating metadata, check log for package errors".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}
