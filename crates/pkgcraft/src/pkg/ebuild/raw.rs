use std::sync::{Arc, OnceLock};
use std::time::Duration;
use std::{fmt, fs};

use camino::Utf8PathBuf;
use indexmap::IndexMap;

use crate::bash;
use crate::dep::{Cpv, Dep};
use crate::eapi::{self, Eapi};
use crate::error::Error;
use crate::macros::bool_not_equal;
use crate::pkg::{make_pkg_traits, Package, RepoPackage};
use crate::repo::ebuild::cache::{Cache, CacheEntry};
use crate::repo::{EbuildRepo, Repository};
use crate::traits::{FilterLines, Intersects};

use super::metadata::Metadata;

#[derive(Clone)]
struct InternalEbuildRawPkg {
    pub(super) cpv: Cpv,
    pub(super) repo: EbuildRepo,
    pub(super) eapi: &'static Eapi,
    data: String,
    chksum: String,
    tree: OnceLock<bash::Tree<'static>>,
}

#[derive(Clone)]
pub struct EbuildRawPkg(Arc<InternalEbuildRawPkg>);

make_pkg_traits!(EbuildRawPkg);

impl fmt::Debug for EbuildRawPkg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "EbuildRawPkg {{ {self} }}")
    }
}

impl EbuildRawPkg {
    pub(crate) fn try_new(cpv: Cpv, repo: &EbuildRepo) -> crate::Result<Self> {
        let relpath = cpv.relpath();
        let data =
            fs::read_to_string(repo.path().join(&relpath)).map_err(|e| Error::InvalidPkg {
                cpv: Box::new(cpv.clone()),
                repo: repo.to_string(),
                err: Box::new(Error::InvalidValue(format!(
                    "failed reading ebuild: {relpath}: {e}"
                ))),
            })?;

        let eapi = Self::parse_eapi(&data).map_err(|error| Error::InvalidPkg {
            cpv: Box::new(cpv.clone()),
            repo: repo.to_string(),
            err: Box::new(error),
        })?;

        let chksum = repo.metadata().cache().chksum(&data);
        let repo = repo.clone();
        let tree = Default::default();
        Ok(Self(Arc::new(InternalEbuildRawPkg {
            cpv,
            repo,
            eapi,
            data,
            chksum,
            tree,
        })))
    }

    /// Get the parsed EAPI from the given ebuild data content.
    fn parse_eapi(data: &str) -> crate::Result<&'static Eapi> {
        let s = data
            .filter_lines()
            .next()
            .and_then(|(_, s)| s.strip_prefix("EAPI="))
            .map(|s| {
                s.split_once('#')
                    .map(|(v, _)| v.trim())
                    .unwrap_or_else(|| s.trim())
            })
            .unwrap_or("0");

        eapi::parse_value(s)?.parse()
    }

    /// Return the path of the package's ebuild relative to the repository root.
    pub fn relpath(&self) -> Utf8PathBuf {
        self.0.cpv.relpath()
    }

    /// Return the absolute path of the package's ebuild.
    pub fn path(&self) -> Utf8PathBuf {
        self.0.repo.path().join(self.relpath())
    }

    /// Return the package's ebuild file content.
    pub fn data(&self) -> &str {
        &self.0.data
    }

    /// Return the checksum of the package's ebuild file content.
    pub fn chksum(&self) -> &str {
        &self.0.chksum
    }

    /// Return the bash parse tree for the ebuild.
    pub fn tree(&self) -> &bash::Tree<'static> {
        self.0.tree.get_or_init(|| {
            // HACK: figure out better method for self-referential lifetimes
            let data: &'static [u8] = unsafe { std::mem::transmute(self.0.data.as_bytes()) };
            bash::Tree::new(data)
        })
    }

    /// Try to deserialize the package's metadata from the cache.
    ///
    /// Optionally, try to regenerate it and update the cache on failure.
    pub(crate) fn metadata(&self, regen_on_failure: bool) -> crate::Result<Metadata> {
        let repo = &self.0.repo;

        // get and deserialize raw metadata cache entry
        let get_metadata = || {
            if let Some(result) = repo.metadata().cache().get(self) {
                result.and_then(|entry| entry.to_metadata(self))
            } else {
                Err(Error::InvalidValue(format!("{self}: missing metadata entry")))
            }
        };

        get_metadata().or_else(|e| {
            if regen_on_failure {
                repo.pool()
                    .metadata_task(repo)
                    .force(true)
                    .run(&self.0.cpv)?;
                get_metadata()
            } else {
                Err(e)
            }
        })
    }

    /// Return the mapping of global environment variables exported by the package.
    pub fn env(&self) -> crate::Result<IndexMap<String, String>> {
        let repo = &self.0.repo;
        self.0.repo.pool().env(repo, &self.0.cpv)
    }

    /// Return the time duration required to source the package.
    pub fn duration(&self) -> crate::Result<Duration> {
        let repo = &self.0.repo;
        self.0.repo.pool().duration(repo, &self.0.cpv)
    }
}

impl Package for EbuildRawPkg {
    fn eapi(&self) -> &'static Eapi {
        self.0.eapi
    }

    fn cpv(&self) -> &Cpv {
        &self.0.cpv
    }
}

impl RepoPackage for EbuildRawPkg {
    type Repo = EbuildRepo;

    fn repo(&self) -> Self::Repo {
        self.0.repo.clone()
    }
}

impl Intersects<Dep> for EbuildRawPkg {
    fn intersects(&self, dep: &Dep) -> bool {
        bool_not_equal!(self.cpn(), dep.cpn());

        if dep.slot_dep().is_some() {
            return false;
        }

        if dep.use_deps().is_some() {
            return false;
        }

        if let Some(val) = dep.repo() {
            bool_not_equal!(self.0.repo.name(), val);
        }

        if let Some(val) = dep.version() {
            self.cpv().version().intersects(val)
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::eapi::EAPI8;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::test::test_data;

    use super::*;

    #[test]
    fn display_and_debug() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();
        let pkg = repo.iter_raw().next().unwrap().unwrap();
        let s = pkg.to_string();
        assert!(format!("{pkg:?}").contains(&s));
    }

    #[test]
    fn relpath() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();
        let raw_pkg = repo.get_pkg_raw("optional/none-8").unwrap();
        assert_eq!(raw_pkg.relpath(), "optional/none/none-8.ebuild");
    }

    #[test]
    fn path() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();
        let raw_pkg = repo.get_pkg_raw("optional/none-8").unwrap();
        assert_eq!(raw_pkg.path(), repo.path().join("optional/none/none-8.ebuild"));
    }

    #[test]
    fn data() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing data content"
            SLOT=0
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        assert_eq!(raw_pkg.data(), data);
        assert!(!raw_pkg.chksum().is_empty());
    }

    #[test]
    fn traits() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();
        let raw_pkg = repo.get_pkg_raw("optional/none-8").unwrap();
        assert_eq!(raw_pkg.eapi(), &*EAPI8);
        assert_eq!(raw_pkg.cpv().to_string(), "optional/none-8");
        assert_eq!(&raw_pkg.repo(), repo);
    }

    #[test]
    fn intersects_dep() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();

        for (s, expected) in [
            ("cat/pkg", true),
            ("=cat/pkg-0", false),
            ("=cat/pkg-1", true),
            ("cat/pkg:0", false),
            ("cat/pkg:0/1", false),
            ("cat/pkg[u]", false),
            ("cat/pkg::test", false),
            ("cat/pkg::commands", true),
        ] {
            let dep: Dep = s.parse().unwrap();
            assert_eq!(raw_pkg.intersects(&dep), expected, "failed for {s}");
        }
    }
}
