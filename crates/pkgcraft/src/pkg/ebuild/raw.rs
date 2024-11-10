use std::{fmt, fs};

use camino::Utf8PathBuf;

use crate::dep::{Cpv, Dep};
use crate::eapi::{self, Eapi};
use crate::macros::bool_not_equal;
use crate::pkg::{make_pkg_traits, Package, RepoPackage};
use crate::repo::ebuild::cache::{Cache, CacheEntry};
use crate::repo::{ebuild::EbuildRepo, Repository};
use crate::traits::{FilterLines, Intersects};
use crate::Error;

use super::metadata::{Metadata, MetadataRaw};

#[derive(Clone)]
pub struct Pkg {
    pub(super) cpv: Cpv,
    pub(super) repo: EbuildRepo,
    pub(super) eapi: &'static Eapi,
    data: String,
    chksum: String,
}

make_pkg_traits!(Pkg);

impl fmt::Debug for Pkg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Pkg {{ {self} }}")
    }
}

impl Pkg {
    pub(crate) fn try_new(cpv: Cpv, repo: EbuildRepo) -> crate::Result<Self> {
        let relpath = cpv.relpath();
        let data = fs::read_to_string(repo.path().join(&relpath)).map_err(|e| {
            Error::IO(format!("{}: failed reading ebuild: {relpath}: {e}", repo.id()))
        })?;

        let eapi = Self::parse_eapi(&data).map_err(|e| Error::InvalidPkg {
            id: format!("{cpv}::{repo}"),
            err: e.to_string(),
        })?;

        let chksum = repo.metadata().cache().chksum(&data);
        Ok(Self { cpv, repo, eapi, data, chksum })
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
        self.cpv.relpath()
    }

    /// Return the absolute path of the package's ebuild.
    pub fn path(&self) -> Utf8PathBuf {
        self.repo.path().join(self.relpath())
    }

    /// Return the package's ebuild file content.
    pub fn data(&self) -> &str {
        &self.data
    }

    /// Return the checksum of the package's ebuild file content.
    pub fn chksum(&self) -> &str {
        &self.chksum
    }

    /// Load raw metadata from the cache if valid, otherwise source it from the ebuild.
    pub fn metadata_raw(&self) -> crate::Result<MetadataRaw> {
        self.repo
            .metadata()
            .cache()
            .get(self)
            .map(|c| c.into_metadata_raw())
            .or_else(|_| self.try_into())
            .map_err(|e| Error::InvalidPkg {
                id: self.to_string(),
                err: e.to_string(),
            })
    }

    /// Load metadata from the cache if valid, otherwise source it from the ebuild.
    pub(crate) fn metadata(&self) -> crate::Result<Metadata> {
        self.repo
            .metadata()
            .cache()
            .get(self)
            .and_then(|c| c.to_metadata(self))
            .or_else(|_| self.try_into())
            .map_err(|e| Error::InvalidPkg {
                id: self.to_string(),
                err: e.to_string(),
            })
    }
}

impl Package for Pkg {
    fn eapi(&self) -> &'static Eapi {
        self.eapi
    }

    fn cpv(&self) -> &Cpv {
        &self.cpv
    }
}

impl RepoPackage for Pkg {
    type Repo = EbuildRepo;

    fn repo(&self) -> Self::Repo {
        self.repo.clone()
    }
}

impl Intersects<Dep> for Pkg {
    fn intersects(&self, dep: &Dep) -> bool {
        bool_not_equal!(self.cpn(), dep.cpn());

        if dep.slot_dep().is_some() {
            return false;
        }

        if dep.use_deps().is_some() {
            return false;
        }

        if let Some(val) = dep.repo() {
            bool_not_equal!(self.repo.name(), val);
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
    use crate::test::TEST_DATA;

    use super::*;

    #[test]
    fn display_and_debug() {
        let repo = TEST_DATA.ebuild_repo("metadata").unwrap();
        let pkg = repo.iter_raw().next().unwrap();
        let s = pkg.to_string();
        assert!(format!("{pkg:?}").contains(&s));
    }

    #[test]
    fn relpath() {
        let repo = TEST_DATA.ebuild_repo("metadata").unwrap();
        let raw_pkg = repo.get_pkg_raw("optional/none-8").unwrap();
        assert_eq!(raw_pkg.relpath(), "optional/none/none-8.ebuild");
    }

    #[test]
    fn path() {
        let repo = TEST_DATA.ebuild_repo("metadata").unwrap();
        let raw_pkg = repo.get_pkg_raw("optional/none-8").unwrap();
        assert_eq!(raw_pkg.path(), repo.path().join("optional/none/none-8.ebuild"));
    }

    #[test]
    fn data() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing data content"
            SLOT=0
        "#};
        let raw_pkg = temp.create_raw_pkg_from_str("cat/pkg-1", data).unwrap();
        assert_eq!(raw_pkg.data(), data);
        assert!(!raw_pkg.chksum().is_empty());
    }

    #[test]
    fn traits() {
        let repo = TEST_DATA.ebuild_repo("metadata").unwrap();
        let raw_pkg = repo.get_pkg_raw("optional/none-8").unwrap();
        assert_eq!(raw_pkg.eapi(), &*EAPI8);
        assert_eq!(raw_pkg.cpv().to_string(), "optional/none-8");
        assert_eq!(&raw_pkg.repo(), repo);
    }

    #[test]
    fn intersects_dep() {
        let repo = TEST_DATA.ebuild_repo("commands").unwrap();
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
