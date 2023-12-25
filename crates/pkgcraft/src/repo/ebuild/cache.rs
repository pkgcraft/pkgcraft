use std::collections::HashSet;
use std::fs;

use camino::Utf8PathBuf;
use indexmap::IndexSet;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use scallop::pool::PoolSendIter;
use strum::{Display, EnumString};
use tracing::error;
use walkdir::WalkDir;

use crate::dep::Cpv;
use crate::error::{Error, PackageError};
use crate::files::{atomic_write_file, is_file, is_hidden};
use crate::pkg::{ebuild::raw::Pkg, Package};
use crate::repo::PkgRepository;
use crate::shell::metadata::Metadata;
use crate::utils::bounded_jobs;

use super::Repo;

#[derive(Display, EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
#[strum(serialize_all = "kebab-case")]
pub enum CacheFormat {
    Md5Dict,
}

pub struct CacheBuilder<'a> {
    repo: &'a Repo,
    jobs: usize,
    force: bool,
    progress: bool,
    suppress: bool,
    cache_path: Utf8PathBuf,
}

impl<'a> CacheBuilder<'a> {
    pub fn new(repo: &'a Repo) -> Self {
        Self {
            repo,
            jobs: num_cpus::get(),
            force: false,
            progress: false,
            suppress: true,
            cache_path: repo.metadata().cache_path().to_path_buf(),
        }
    }

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

    /// Set the cache path to use for file output.
    pub fn cache_path<S: AsRef<str>>(mut self, value: S) -> Self {
        let path = value.as_ref();
        if !path.is_empty() {
            self.cache_path = Utf8PathBuf::from(path);
        }
        self
    }

    /// Regenerate the package metadata cache, returning the number of errors that occurred.
    pub fn run(&self) -> crate::Result<()> {
        // initialize pool first to minimize forked process memory pages
        let func = |cpv: Cpv<String>| -> scallop::Result<()> {
            let pkg = &Pkg::try_new(cpv, self.repo)?;
            // convert raw pkg into metadata via sourcing
            let meta: Metadata = pkg.try_into().map_err(|e| pkg.invalid_pkg_err(e))?;

            // determine metadata entry directory
            let dir = self.cache_path.join(pkg.cpv().category());

            // atomically create metadata file
            let pf = pkg.pf();
            let path = dir.join(format!(".{pf}"));
            let new_path = dir.join(pf);
            atomic_write_file(&path, meta.to_bytes(), &new_path)?;
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
        let mut cpvs: IndexSet<_> = self
            .repo
            .categories()
            .into_par_iter()
            .flat_map(|s| self.repo.category_cpvs(&s))
            .collect();

        // set progression length encompassing all pkgs
        pb.set_length(cpvs.len().try_into().unwrap());

        if self.cache_path.exists() {
            // TODO: replace with parallelized cache iterator
            let entries: Vec<_> = WalkDir::new(&self.cache_path)
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
                        .strip_prefix(&self.cache_path)
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
                        Pkg::metadata_regen(cpv, self.repo, &self.cache_path)
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
                let path = self.cache_path.join(cat);
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
