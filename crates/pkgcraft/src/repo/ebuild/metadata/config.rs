use std::str::{FromStr, SplitWhitespace};
use std::{fmt, fs, io};

use camino::Utf8Path;
use itertools::Itertools;

use crate::Error;
use crate::files::atomic_write_file;
use crate::pkg::ebuild::manifest::HashType;
use crate::repo::Repository;
use crate::repo::ebuild::cache::CacheFormat;
use crate::types::OrderedSet;

/// Wrapper for ini format config files.
#[derive(Debug, Default)]
struct Ini(ini::Ini);

impl Ini {
    fn load(path: &Utf8Path) -> crate::Result<Self> {
        let data = match fs::read_to_string(path) {
            Ok(data) => data,
            Err(e) if e.kind() == io::ErrorKind::NotFound => Default::default(),
            Err(e) => return Err(Error::IO(e.to_string())),
        };

        data.parse()
    }

    /// Iterate over the config values for a key, splitting by whitespace.
    fn iter(&self, key: &str) -> SplitWhitespace<'_> {
        self.get(key).unwrap_or_default().split_whitespace()
    }

    /// Get a value related to a key from the main section if it exists.
    fn get(&self, key: &str) -> Option<&str> {
        self.0.general_section().get(key)
    }
}

impl FromStr for Ini {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let ini = ini::Ini::load_from_str(s)
            .map_err(|e| Error::InvalidValue(format!("failed parsing INI: {e}")))?;
        Ok(Self(ini))
    }
}

/// Ebuild repo configuration as defined by GLEP 82.
#[derive(Debug, Default, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub struct Config {
    /// The ordered set of metadata cache types.
    pub cache_formats: OrderedSet<CacheFormat>,

    /// The ordered set of banned EAPIs.
    pub eapis_banned: OrderedSet<String>,

    /// The ordered set of deprecated EAPIs.
    pub eapis_deprecated: OrderedSet<String>,

    /// The ordered set of unstable EAPIs.
    pub eapis_testing: OrderedSet<String>,

    /// The ordered set of hash types that should be used for Manifest entries.
    pub manifest_hashes: OrderedSet<HashType>,

    /// The ordered set of hash types that must be used for Manifest entries.
    pub manifest_required_hashes: OrderedSet<HashType>,

    /// The ordered set of inherited repo ids.
    pub masters: OrderedSet<String>,

    /// The ordered set of custom extensions enabled for profiles.
    pub profile_formats: OrderedSet<String>,

    /// Allowed values for ebuild PROPERTIES.
    pub properties_allowed: OrderedSet<String>,

    /// Allowed values for ebuild RESTRICT.
    pub restrict_allowed: OrderedSet<String>,

    /// Control whether thin or thick Manifest files are used.
    pub thin_manifests: bool,
}

/// Parse an iterable value from an [`Ini`] object.
macro_rules! parse_iter {
    ($ini:expr, $key:expr) => {
        $ini.iter($key)
            .map(|s| {
                s.parse().map_err(|_| {
                    Error::InvalidValue(format!("{}: unsupported value: {s}", $key))
                })
            })
            .try_collect()
    };
}

/// Parse a value from an [`Ini`] object.
macro_rules! parse {
    ($ini:expr, $key:expr) => {
        $ini.get($key)
            .map(|s| {
                s.parse().map_err(|_| {
                    Error::InvalidValue(format!("{}: unsupported value: {s}", $key))
                })
            })
            .transpose()
    };
}

impl TryFrom<Ini> for Config {
    type Error = Error;

    fn try_from(ini: Ini) -> crate::Result<Self> {
        Ok(Self {
            cache_formats: parse_iter!(ini, "cache-formats")?,
            eapis_banned: parse_iter!(ini, "eapis-banned")?,
            eapis_deprecated: parse_iter!(ini, "eapis-deprecated")?,
            eapis_testing: parse_iter!(ini, "eapis-testing")?,
            manifest_hashes: parse_iter!(ini, "manifest-hashes")?,
            manifest_required_hashes: parse_iter!(ini, "manifest-required-hashes")?,
            masters: parse_iter!(ini, "masters")?,
            profile_formats: parse_iter!(ini, "profile-formats")?,
            properties_allowed: parse_iter!(ini, "properties-allowed")?,
            restrict_allowed: parse_iter!(ini, "restrict-allowed")?,
            thin_manifests: parse!(ini, "thin-manifests")?.unwrap_or(false),
        })
    }
}

impl Config {
    pub(super) fn try_new<P: AsRef<Utf8Path>>(repo_path: P) -> crate::Result<Self> {
        let path = repo_path.as_ref().join("metadata/layout.conf");
        let ini = Ini::load(&path)?;
        ini.try_into()
    }

    /// The config file contains no settings or is nonexistent.
    pub fn is_empty(&self) -> bool {
        self == &Default::default()
    }

    /// Write the config back to a given repo.
    pub fn write<R: Repository>(&self, repo: R) -> crate::Result<()> {
        let path = repo.path().join("metadata/layout.conf");
        let data = self.to_string();
        atomic_write_file(&path, &data)
    }
}

impl FromStr for Config {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Ini::from_str(s)?.try_into()
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if !self.cache_formats.is_empty() {
            let values = self.cache_formats.iter().join(" ");
            writeln!(f, "cache-formats = {values}")?;
        }
        if !self.eapis_banned.is_empty() {
            let values = self.eapis_banned.iter().join(" ");
            writeln!(f, "eapis-banned = {values}")?;
        }
        if !self.eapis_deprecated.is_empty() {
            let values = self.eapis_deprecated.iter().join(" ");
            writeln!(f, "eapis-deprecated = {values}")?;
        }
        if !self.eapis_testing.is_empty() {
            let values = self.eapis_testing.iter().join(" ");
            writeln!(f, "eapis-testing = {values}")?;
        }
        if !self.manifest_hashes.is_empty() {
            let values = self.manifest_hashes.iter().join(" ");
            writeln!(f, "manifest-hashes = {values}")?;
        }
        if !self.manifest_required_hashes.is_empty() {
            let values = self.manifest_required_hashes.iter().join(" ");
            writeln!(f, "manifest-required-hashes = {values}")?;
        }
        if !self.masters.is_empty() {
            let values = self.masters.iter().join(" ");
            writeln!(f, "masters = {values}")?;
        }
        if !self.profile_formats.is_empty() {
            let values = self.profile_formats.iter().join(" ");
            writeln!(f, "profile-formats = {values}")?;
        }
        if !self.properties_allowed.is_empty() {
            let values = self.properties_allowed.iter().join(" ");
            writeln!(f, "properties-allowed = {values}")?;
        }
        if !self.restrict_allowed.is_empty() {
            let values = self.restrict_allowed.iter().join(" ");
            writeln!(f, "restrict-allowed = {values}")?;
        }
        writeln!(f, "thin-manifests = {}", self.thin_manifests)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::test::assert_ordered_eq;

    use super::*;

    #[test]
    fn settings() {
        // empty
        let config = Config::default();
        assert!(config.is_empty());
        assert!(config.masters.is_empty());
        assert!(config.properties_allowed.is_empty());
        assert!(config.restrict_allowed.is_empty());
        assert!(!config.thin_manifests);

        // valid
        let data = indoc::indoc! {r#"
            masters = repo1 repo2
            properties-allowed = interactive live
            restrict-allowed = fetch mirror
            thin-manifests = false
        "#};
        let config: Config = data.parse().unwrap();
        assert_ordered_eq!(&config.masters, ["repo1", "repo2"]);
        assert_ordered_eq!(&config.properties_allowed, ["interactive", "live"]);
        assert_ordered_eq!(&config.restrict_allowed, ["fetch", "mirror"]);
        assert!(!config.thin_manifests);
        assert_eq!(config.to_string(), data);
    }
}
