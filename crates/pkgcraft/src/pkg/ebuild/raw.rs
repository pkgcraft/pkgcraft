use std::fs;

use camino::{Utf8Path, Utf8PathBuf};

use crate::dep::Cpv;
use crate::eapi::{self, Eapi};
use crate::pkg::{make_pkg_traits, Package, RepoPackage};
use crate::repo::{ebuild::Repo, Repository};
use crate::shell::metadata::{Metadata, MetadataRaw};
use crate::traits::FilterLines;
use crate::utils::digest;
use crate::Error;

#[derive(Debug)]
pub struct Pkg<'a> {
    pub(super) cpv: Cpv<String>,
    pub(super) repo: &'a Repo,
    pub(super) eapi: &'static Eapi,
    data: String,
    chksum: String,
}

make_pkg_traits!(Pkg<'_>);

impl<'a> Pkg<'a> {
    pub(crate) fn try_new(cpv: Cpv<String>, repo: &'a Repo) -> crate::Result<Self> {
        let relpath = cpv.relpath();
        let data = fs::read_to_string(repo.path().join(&relpath)).map_err(|e| {
            Error::IO(format!("{}: failed reading ebuild: {relpath}: {e}", repo.id()))
        })?;

        let eapi = Self::parse_eapi(&data).map_err(|e| Error::InvalidPkg {
            id: format!("{cpv}::{repo}"),
            err: e.to_string(),
        })?;

        let chksum = digest::<md5::Md5>(data.as_bytes());
        Ok(Self { cpv, repo, eapi, data, chksum })
    }

    /// Get the parsed EAPI from the given ebuild data content.
    fn parse_eapi(data: &str) -> crate::Result<&'static Eapi> {
        data.filter_lines()
            .next()
            .and_then(|(_, s)| s.strip_prefix("EAPI="))
            .map(|s| {
                s.split_once('#')
                    .map(|(v, _)| v.trim())
                    .unwrap_or_else(|| s.trim())
            })
            .ok_or_else(|| Error::InvalidValue("unsupported EAPI: 0".to_string()))
            .and_then(eapi::parse_value)
            .and_then(TryInto::try_into)
    }

    /// Return the path of the package's ebuild relative to the repository root.
    pub fn relpath(&self) -> Utf8PathBuf {
        self.cpv.relpath()
    }

    /// Return the absolute path of the package's ebuild.
    pub fn abspath(&self) -> Utf8PathBuf {
        self.repo.path().join(self.relpath())
    }

    /// Return the package's ebuild as a string.
    pub fn data(&self) -> &str {
        &self.data
    }

    /// Return the checksum of the package.
    pub fn chksum(&self) -> &str {
        &self.chksum
    }

    /// Check if a package's metadata requires regeneration.
    pub(crate) fn metadata_regen(cpv: &Cpv<String>, repo: &'a Repo, cache_path: &Utf8Path) -> bool {
        Self::try_new(cpv.clone(), repo)
            .and_then(|pkg| pkg.metadata_raw(cache_path))
            .is_err()
    }

    /// Load raw metadata and verify its validity.
    pub(crate) fn metadata_raw(&self, cache_path: &Utf8Path) -> crate::Result<MetadataRaw> {
        let meta = MetadataRaw::load(self, cache_path)?;
        meta.verify(self)?;
        Ok(meta)
    }

    /// Load metadata from the cache if valid, otherwise source it from the ebuild.
    pub(crate) fn metadata(&self, cache_path: &Utf8Path) -> crate::Result<Metadata<'a>> {
        self.metadata_raw(cache_path)
            .and_then(|m| m.deserialize(self))
            .or_else(|_| self.try_into())
            .map_err(|e| Error::InvalidPkg {
                id: self.to_string(),
                err: e.to_string(),
            })
    }
}

impl<'a> Package for Pkg<'a> {
    fn eapi(&self) -> &'static Eapi {
        self.eapi
    }

    fn cpv(&self) -> &Cpv<String> {
        &self.cpv
    }
}

impl<'a> RepoPackage for Pkg<'a> {
    type Repo = &'a Repo;

    fn repo(&self) -> Self::Repo {
        self.repo
    }
}
