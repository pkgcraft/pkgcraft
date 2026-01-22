use camino::Utf8Path;
use indexmap::IndexSet;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use tracing::error;

use crate::dep::Cpv;
use crate::error::Error;
use crate::pkg::ebuild::{EbuildRawPkg, Metadata, MetadataKey};
use crate::repo::PkgRepository;
use crate::restrict::Restrict;
use crate::shell::pool::MetadataRegen;
use crate::traits::Contains;

use super::EbuildRepo;

pub(crate) mod md5_dict;

pub trait CacheEntry {
    /// Deserialize a cache entry to package metadata.
    fn to_metadata(&self, pkg: &EbuildRawPkg) -> crate::Result<Metadata>;

    /// Verify a cache entry is valid.
    fn verify(&self, pkg: &EbuildRawPkg) -> crate::Result<()>;

    /// Return the raw value for a given metadata key.
    fn get(&self, key: &MetadataKey) -> Option<&str>;
}

pub trait Cache {
    type Entry: CacheEntry;

    /// Return the hex-encoded checksum for the given data.
    fn chksum<S: AsRef<[u8]>>(&self, data: S) -> String;

    /// Return the cache's format.
    fn format(&self) -> CacheFormat;

    /// Return the cache's filesystem path.
    fn path(&self) -> &Utf8Path;

    /// Get the cache entry for a given package if it exists and is valid.
    fn get(&self, pkg: &EbuildRawPkg) -> crate::Result<Self::Entry>;

    /// Update the cache with the given package metadata.
    fn update(&self, pkg: &EbuildRawPkg, meta: &Metadata) -> crate::Result<()>;

    /// Forcibly remove the entire cache.
    fn remove(&self, repo: &EbuildRepo) -> crate::Result<()>;

    /// Remove a cache entry erroring if nonexistent.
    fn remove_entry<T>(&self, value: T) -> crate::Result<()>
    where
        T: TryInto<Cpv>,
        Error: From<T::Error>;

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

    fn get(&self, key: &MetadataKey) -> Option<&str> {
        match self {
            Self::Md5Dict(entry) => entry.get(key),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            Self::Md5Dict(cache) => cache.get(pkg).map(Self::Entry::Md5Dict),
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

    fn remove_entry<T>(&self, value: T) -> crate::Result<()>
    where
        T: TryInto<Cpv>,
        Error: From<T::Error>,
    {
        match self {
            Self::Md5Dict(cache) => cache.remove_entry(value),
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
    pub fn regen(&self, repo: &EbuildRepo) -> MetadataCacheRegen<'_> {
        MetadataCacheRegen {
            cache: self,
            progress: false,
            clean: true,
            regen: repo.metadata_regen().cache(self),
            repo: repo.clone(),
            targets: None,
        }
    }
}

#[derive(Debug)]
pub struct MetadataCacheRegen<'a> {
    cache: &'a MetadataCache,
    progress: bool,
    clean: bool,
    regen: MetadataRegen,
    repo: EbuildRepo,
    targets: Option<IndexSet<Cpv>>,
}

impl MetadataCacheRegen<'_> {
    /// Force metadata regeneration across all packages.
    pub fn force(mut self, value: bool) -> Self {
        self.regen = self.regen.force(value);
        self
    }

    /// Show a progress bar during cache regeneration.
    pub fn progress(mut self, value: bool) -> Self {
        self.progress = value;
        self
    }

    /// Allow output from stdout and stderr during cache regeneration.
    pub fn output(mut self, value: bool) -> Self {
        self.regen = self.regen.output(value);
        self
    }

    /// Perform metadata verification without writing to the cache.
    pub fn verify(mut self, value: bool) -> Self {
        self.regen = self.regen.verify(value);
        self.clean = false;
        self
    }

    /// Specify package targets for cache regeneration.
    pub fn targets(mut self, restrict: Restrict) -> Self {
        // TODO: use parallel Cpv restriction iterator
        // skip repo level targets that needlessly slow down regen
        if restrict != Restrict::True {
            self.targets = Some(self.repo.iter_cpv_restrict(restrict).collect());
        }
        self.clean = false;
        self
    }

    /// Regenerate the package metadata cache.
    pub fn run(self) -> crate::Result<()> {
        let cpvs = self.targets.unwrap_or_else(|| {
            // TODO: replace with parallel Cpv iterator -- repo.par_iter_cpvs()
            // pull all package Cpvs from the repo
            self.repo
                .categories()
                .into_par_iter()
                .flat_map(|s| self.repo.cpvs_from_category(&s))
                .collect()
        });

        // track progress encompassing all targets
        let progress = if self.progress {
            ProgressBar::new(cpvs.len().try_into().unwrap()).with_style(
                ProgressStyle::with_template("{wide_bar} {msg} {pos}/{len}").unwrap(),
            )
        } else {
            ProgressBar::hidden()
        };

        // remove outdated cache entries
        if self.clean {
            self.cache.clean(&cpvs)?;
        }

        // hack to force log capturing for tests to work in threads
        // https://github.com/dbrgn/tracing-test/issues/23
        #[cfg(test)]
        let thread_span = tracing::debug_span!("thread").or_current();

        // run cache verification in a thread pool that runs blocking metadata tasks
        // in build pool processes as necessary
        let errors = cpvs
            .into_par_iter()
            .map(|cpv| {
                progress.inc(1);
                self.regen.get(cpv)
            })
            .inspect(|result| {
                match result {
                    Err(e) => {
                        // hack to force log capturing for tests to work in threads
                        // https://github.com/dbrgn/tracing-test/issues/23
                        #[cfg(test)]
                        let _entered = thread_span.clone().entered();

                        progress.suspend(|| {
                            error!("{e}");
                        });
                    }
                    Ok(Some(output)) => progress.suspend(|| eprintln!("{output}")),
                    Ok(None) => (),
                }
            })
            .filter(|result| result.is_err())
            .count();

        progress.finish_and_clear();
        if errors > 0 {
            Err(Error::InvalidValue("metadata failures occurred".to_string()))
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
    use crate::test::{assert_err_re, assert_logs_re};

    use super::*;

    #[traced_test]
    #[test]
    fn regen_errors() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();

        // create a large number of packages with a subshelled, invalid scope builtin call
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing metadata generation error handling"
            SLOT=0
            VAR=$(best_version cat/pkg)
        "#};
        for pv in 0..50 {
            temp.create_ebuild_from_str(format!("cat/pkg-{pv}"), data)
                .unwrap();
        }

        // run regen asserting that errors occurred
        let r = repo.metadata().cache().regen(&repo).progress(true).run();
        assert!(r.is_err());

        // verify all pkgs caused logged errors
        for pv in 0..50 {
            assert_logs_re!(
                "invalid pkg: cat/pkg-{pv}::test: line 4: best_version: error: disabled in global scope$"
            );
        }
    }

    #[test]
    fn cache() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        temp.create_ebuild("cat/pkg-1", &[]).unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();

        let pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        let cache = repo.metadata().cache();

        // cache entry doesn't exist
        assert!(cache.get(&pkg).is_err());

        // generate cache
        cache.regen(&repo).run().unwrap();

        // valid cache entry exists
        let entry = cache.get(&pkg).unwrap();
        assert!(entry.verify(&pkg).is_ok());
        assert!(entry.get(&MetadataKey::DEPEND).is_none());
        assert_eq!(entry.get(&MetadataKey::SLOT).unwrap(), "0");

        // remove nonexistent cache entry
        let r = cache.remove_entry("cat/pkg-2");
        assert_err_re!(r, "^failed removing cache file: cat/pkg-2: No such file or directory");

        // remove existent cache entry
        cache.remove_entry("cat/pkg-1").unwrap();

        // cache entry doesn't exist
        assert!(cache.get(&pkg).is_err());
    }
}
