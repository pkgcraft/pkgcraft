use camino::Utf8Path;
use indexmap::IndexSet;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use strum::{Display, EnumString};
use tracing::error;

use crate::dep::Cpv;
use crate::error::Error;
use crate::pkg::ebuild::metadata::Metadata;
use crate::pkg::ebuild::EbuildRawPkg;
use crate::repo::{PkgRepository, Repository};
use crate::traits::Contains;
use crate::utils::bounded_jobs;

use super::EbuildRepo;

pub(crate) mod md5_dict;

pub trait CacheEntry {
    /// Deserialize a cache entry to package metadata.
    fn to_metadata(&self, pkg: &EbuildRawPkg) -> crate::Result<Metadata>;
    /// Verify a cache entry is valid.
    fn verify(&self, pkg: &EbuildRawPkg) -> crate::Result<()>;
}

pub trait Cache {
    type Entry: CacheEntry;
    /// Return the hex-encoded checksum for the given data.
    fn chksum<S: AsRef<[u8]>>(&self, data: S) -> String;
    /// Return the cache's format.
    fn format(&self) -> CacheFormat;
    /// Return the cache's filesystem path.
    fn path(&self) -> &Utf8Path;
    /// Get the cache entry for a given package.
    fn get(&self, pkg: &EbuildRawPkg) -> crate::Result<Self::Entry>;
    /// Update the cache with the given package metadata.
    fn update(&self, pkg: &EbuildRawPkg, meta: &Metadata) -> crate::Result<()>;
    /// Forcibly remove the entire cache.
    fn remove(&self, repo: &EbuildRepo) -> crate::Result<()>;
    /// Remove outdated entries from the cache.
    fn clean<C: for<'a> Contains<&'a Cpv> + Sync>(&self, collection: C) -> crate::Result<()>;
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
    pub fn from_repo<P: AsRef<Utf8Path>>(&self, path: P) -> MetadataCache {
        match self {
            Self::Md5Dict => MetadataCache::Md5Dict(md5_dict::Md5Dict::from_repo(path)),
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
    fn to_metadata(&self, pkg: &EbuildRawPkg) -> crate::Result<Metadata> {
        match self {
            Self::Md5Dict(entry) => entry.to_metadata(pkg),
        }
    }

    fn verify(&self, pkg: &EbuildRawPkg) -> crate::Result<()> {
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

    fn chksum<S: AsRef<[u8]>>(&self, data: S) -> String {
        match self {
            Self::Md5Dict(cache) => cache.chksum(data),
        }
    }

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

    fn get(&self, pkg: &EbuildRawPkg) -> crate::Result<Self::Entry> {
        match self {
            Self::Md5Dict(cache) => cache.get(pkg).map(MetadataCacheEntry::Md5Dict),
        }
    }

    fn update(&self, pkg: &EbuildRawPkg, meta: &Metadata) -> crate::Result<()> {
        match self {
            Self::Md5Dict(cache) => cache.update(pkg, meta),
        }
    }

    fn remove(&self, repo: &EbuildRepo) -> crate::Result<()> {
        let path = self.path();
        if !path.starts_with(repo.path()) {
            return Err(Error::IO(format!("removal unsupported for external cache: {path}")));
        } else if !path.exists() {
            return Ok(());
        }

        match self {
            Self::Md5Dict(cache) => cache.remove(repo),
        }
    }

    fn clean<C: for<'a> Contains<&'a Cpv> + Sync>(&self, collection: C) -> crate::Result<()> {
        match self {
            Self::Md5Dict(cache) => cache.clean(collection),
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
            progress: ProgressBar::hidden(),
            output: false,
            clean: true,
            verify: false,
            targets: None,
        }
    }
}

#[derive(Debug)]
pub struct MetadataCacheRegen<'a> {
    cache: &'a MetadataCache,
    jobs: usize,
    force: bool,
    progress: ProgressBar,
    output: bool,
    clean: bool,
    verify: bool,
    targets: Option<IndexSet<Cpv>>,
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
        if value {
            self.progress = ProgressBar::new(0);
            self.progress
                .set_style(ProgressStyle::with_template("{wide_bar} {msg} {pos}/{len}").unwrap());
        }
        self
    }

    /// Allow output from stdout and stderr during cache regeneration.
    pub fn output(mut self, value: bool) -> Self {
        self.output = value;
        self
    }

    /// Perform metadata verification without writing to the cache.
    pub fn verify(mut self, value: bool) -> Self {
        self.verify = value;
        self.clean = false;
        self
    }

    /// Specify package targets for cache regeneration.
    pub fn targets<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = Cpv>,
    {
        self.clean = false;
        self.targets = Some(values.into_iter().collect());
        self
    }

    /// Regenerate the package metadata cache, returning the number of errors that occurred.
    pub fn run(self, repo: &EbuildRepo) -> crate::Result<()> {
        let cpvs = self.targets.unwrap_or_else(|| {
            // TODO: replace with parallel Cpv iterator -- repo.par_iter_cpvs()
            // pull all package Cpvs from the repo
            repo.categories()
                .into_par_iter()
                .flat_map(|s| repo.cpvs_from_category(&s))
                .collect()
        });

        // set progression length encompassing all targets
        self.progress.set_length(cpvs.len().try_into().unwrap());

        // remove outdated cache entries
        if self.clean && self.cache.path().exists() {
            self.cache.clean(&cpvs)?;
        }

        let mut errors = 0;
        if !cpvs.is_empty() {
            if self.verify {
                self.progress.set_message("verifying metadata:");
            } else {
                self.progress.set_message("generating metadata:");
            }

            // run cache verification in a thread pool that runs blocking metadata tasks
            // in build pool processes as necessary
            let pool = repo.pool();
            errors = cpvs
                .into_par_iter()
                .map(|cpv| {
                    self.progress.inc(1);
                    pool.metadata(repo, &cpv, self.force, self.verify)
                })
                .filter(|result| {
                    // log errors
                    if let Err(e) = result {
                        error!("{e}");
                        true
                    } else {
                        false
                    }
                })
                .count();
        }

        if errors > 0 {
            Err(Error::InvalidValue("metadata failures occurred, see log for details".to_string()))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use crate::config::Config;
    use crate::repo::ebuild::EbuildRepoBuilder;

    #[traced_test]
    #[test]
    fn regen_errors() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        // create a large number of packages with a subshelled, invalid scope builtin call
        for pv in 0..50 {
            let data = indoc::indoc! {r#"
                EAPI=8
                DESCRIPTION="testing metadata generation error handling"
                SLOT=0
                VAR=$(best_version cat/pkg)
        /   "#};
            temp.create_ebuild_from_str(format!("cat/pkg-{pv}"), data)
                .unwrap();
        }

        // run regen asserting that errors occurred
        let r = repo.metadata().cache().regen().run(&repo);
        assert!(r.is_err());

        // TODO: Skip for now since log capturing doesn't in threads without hacks.
        // https://github.com/dbrgn/tracing-test/issues/23
        //
        // verify all pkgs caused logged errors
        /*for pv in 0..50 {
            assert_logs_re!(format!(
                "invalid pkg: cat/pkg-{pv}::test: line 4: best_version: error: disabled in global scope$"
            ));
        }*/
    }
}
