use std::fs;

use camino::Utf8PathBuf;

use crate::dep::Cpv;
use crate::eapi::{self, Eapi};
use crate::pkg::{make_pkg_traits, Package, RepoPackage};
use crate::repo::{ebuild::Repo, Repository};
use crate::shell::metadata::Metadata;
use crate::traits::FilterLines;
use crate::utils::digest;
use crate::Error;

#[derive(Debug)]
pub struct Pkg<'a> {
    pub(super) cpv: Cpv,
    pub(super) repo: &'a Repo,
    pub(super) eapi: &'static Eapi,
    data: String,
    chksum: String,
}

make_pkg_traits!(Pkg<'_>);

impl<'a> Pkg<'a> {
    pub(crate) fn new(cpv: Cpv, repo: &'a Repo) -> crate::Result<Self> {
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

    /// Load metadata from cache if valid, otherwise source it from the ebuild.
    pub(super) fn load_or_source(&self) -> crate::Result<Metadata> {
        Metadata::load(self, true)
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

    fn cpv(&self) -> &Cpv {
        &self.cpv
    }
}

impl<'a> RepoPackage for Pkg<'a> {
    type Repo = &'a Repo;

    fn repo(&self) -> Self::Repo {
        self.repo
    }
}
