use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::str::{FromStr, SplitWhitespace};
use std::{fmt, fs, io};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{IndexMap, IndexSet};
use once_cell::sync::OnceCell;
use strum::{Display, EnumString};
use tracing::{error, warn};

use crate::dep::{parse, Dep};
use crate::eapi::{Eapi, EAPI0};
use crate::pkg::ebuild::metadata::HashType;
use crate::set::OrderedSet;
use crate::Error;

const DEFAULT_SECTION: Option<String> = None;

/// Wrapper for ini format config files.
struct Ini(ini::Ini);

impl fmt::Debug for Ini {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let section = self.0.section(DEFAULT_SECTION);
        f.debug_tuple("Ini").field(&section).finish()
    }
}

impl Default for Ini {
    fn default() -> Self {
        Self(ini::Ini::new())
    }
}

impl Ini {
    fn load(path: &Utf8Path) -> crate::Result<Self> {
        match ini::Ini::load_from_file(path) {
            Ok(c) => Ok(Self(c)),
            Err(ini::Error::Io(e)) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(Error::IO(e.to_string())),
        }
    }

    /// Iterate over the config values for a given key, splitting by whitespace.
    pub(super) fn iter(&self, key: &str) -> SplitWhitespace {
        self.0
            .get_from(DEFAULT_SECTION, key)
            .unwrap_or_default()
            .split_whitespace()
    }
}

/// Ebuild repo configuration as defined by GLEP 82.
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Config {
    manifest_hashes: OrderedSet<HashType>,
    manifest_required_hashes: OrderedSet<HashType>,
    masters: OrderedSet<String>,
    properties_allowed: OrderedSet<String>,
    restrict_allowed: OrderedSet<String>,
}

impl Config {
    fn new(repo_path: &Utf8Path) -> crate::Result<Self> {
        let path = repo_path.join("metadata/layout.conf");
        let ini = Ini::load(&path)?;

        // convert iterable config values into collection
        let ini_iter = |key: &str| ini.iter(key).map(String::from).collect();

        // convert iterable hash values into collection
        let ini_hashes = |key: &str| -> crate::Result<OrderedSet<_>> {
            ini.iter(key)
                .map(|s| HashType::from_str(s).map_err(|e| Error::InvalidValue(e.to_string())))
                .collect()
        };

        Ok(Self {
            manifest_hashes: ini_hashes("manifest-hashes")?,
            manifest_required_hashes: ini_hashes("manifest-required-hashes")?,
            masters: ini_iter("masters"),
            properties_allowed: ini_iter("properties-allowed"),
            restrict_allowed: ini_iter("restrict-allowed"),
        })
    }

    /// Return the list of hash types that must be used for Manifest entries.
    pub fn manifest_required_hashes(&self) -> &OrderedSet<HashType> {
        &self.manifest_required_hashes
    }

    /// Return the list of hash types that should be used for Manifest entries.
    pub fn manifest_hashes(&self) -> &OrderedSet<HashType> {
        &self.manifest_hashes
    }

    /// Return the list of inherited repo ids.
    pub fn masters(&self) -> &OrderedSet<String> {
        &self.masters
    }

    /// Allowed values for ebuild PROPERTIES.
    pub fn properties_allowed(&self) -> &OrderedSet<String> {
        &self.properties_allowed
    }

    /// Allowed values for ebuild RESTRICT.
    pub fn restrict_allowed(&self) -> &OrderedSet<String> {
        &self.restrict_allowed
    }

    /// The config file contains no settings or is nonexistent.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

trait FilterLines {
    fn filter_lines(&self) -> Box<dyn Iterator<Item = (usize, &str)> + '_>;
}

impl<T: Borrow<str>> FilterLines for T {
    fn filter_lines(&self) -> Box<dyn Iterator<Item = (usize, &str)> + '_> {
        let iter = self
            .borrow()
            .lines()
            .map(|s| s.trim())
            .enumerate()
            .filter(|(_, s)| !s.is_empty() && !s.starts_with('#'));

        Box::new(iter)
    }
}

#[derive(Display, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum ArchStatus {
    Stable,
    Testing,
    Transitional,
}

#[derive(Debug, Default)]
pub struct Metadata {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) eapi: &'static Eapi,
    config: Config,
    path: Utf8PathBuf,
    arches: OnceCell<IndexSet<String>>,
    arches_desc: OnceCell<HashMap<ArchStatus, HashSet<String>>>,
    categories: OnceCell<IndexSet<String>>,
    mirrors: OnceCell<IndexMap<String, IndexSet<String>>>,
    pkg_deprecated: OnceCell<IndexSet<Dep>>,
    pkg_mask: OnceCell<IndexSet<Dep>>,
}

impl Metadata {
    pub(super) fn new(id: &str, path: &Utf8Path) -> crate::Result<Self> {
        let invalid_repo = |err: &str| -> Error {
            Error::InvalidRepo {
                id: id.to_string(),
                err: err.to_string(),
            }
        };

        // verify repo name
        let name = match fs::read_to_string(path.join("profiles/repo_name")) {
            Ok(data) => match data.lines().next().map(|s| parse::repo(s.trim())) {
                Some(Ok(s)) => s.to_string(),
                Some(Err(e)) => return Err(invalid_repo(&format!("profiles/repo_name: {e}"))),
                None => return Err(invalid_repo("profiles/repo_name empty")),
            },
            Err(e) => return Err(invalid_repo(&format!("profiles/repo_name: {e}"))),
        };

        // verify repo EAPI
        let eapi = match fs::read_to_string(path.join("profiles/eapi")) {
            Ok(data) => {
                let s = data.lines().next().unwrap_or_default();
                <&Eapi>::from_str(s.trim_end())
                    .map_err(|e| invalid_repo(&format!("profiles/eapi: {e}")))?
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => &*EAPI0,
            Err(e) => return Err(invalid_repo(&format!("profiles/eapi: {e}"))),
        };

        let config =
            Config::new(path).map_err(|e| invalid_repo(&format!("metadata/layout.conf: {e}")))?;

        Ok(Self {
            id: id.to_string(),
            name,
            eapi,
            config,
            path: Utf8PathBuf::from(path),
            ..Default::default()
        })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Return a repo's known architectures from `profiles/arch.list`.
    pub fn arches(&self) -> &IndexSet<String> {
        self.arches.get_or_init(|| {
            let path = self.path.join("profiles/arch.list");
            match fs::read_to_string(path) {
                Ok(s) => s.filter_lines()
                    .map(|(_, s)| s.to_string())
                    .collect(),
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}::profiles/arch.list: {e}", self.id);
                    }
                    Default::default()
                }
            }
        })
    }

    /// Architecture stability status from `profiles/arches.desc`.
    /// See GLEP 72 (https://www.gentoo.org/glep/glep-0072.html).
    pub fn arches_desc(&self) -> &HashMap<ArchStatus, HashSet<String>> {
        self.arches_desc.get_or_init(|| {
            let path = self.path.join("profiles/arches.desc");
            let mut vals = HashMap::<ArchStatus, HashSet<String>>::new();
            match fs::read_to_string(path) {
                Ok(s) => {
                    s.filter_lines()
                        .map(|(i, s)| (i, s.split_whitespace()))
                        // only pull the first two columns, ignoring any additional
                        .for_each(|(i, mut iter)| match (iter.next(), iter.next()) {
                            (Some(arch), Some(status)) => {
                                if !self.arches().contains(arch) {
                                    warn!(
                                        "{}::profiles/arches.desc, line {}: unknown arch: {arch}",
                                        self.id,
                                        i + 1
                                    );
                                    return;
                                }

                                if let Ok(status) = ArchStatus::from_str(status) {
                                    vals.entry(status)
                                        .or_insert_with(HashSet::new)
                                        .insert(arch.to_string());
                                } else {
                                    warn!(
                                        "{}::profiles/arches.desc, line {}: unknown status: {status}",
                                        self.id, i + 1
                                    );
                                }
                            }
                            _ => error!(
                                "{}::profiles/arches.desc, line {}: \
                                invalid line format: should be '<arch> <status>'",
                                self.id,
                                i + 1
                            ),
                        })
                }
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}::profiles/arches.desc: {e}", self.id);
                    }
                }
            }
            vals
        })
    }

    /// Return a repo's configured categories from `profiles/categories`.
    pub fn categories(&self) -> &IndexSet<String> {
        self.categories.get_or_init(|| {
            let path = self.path.join("profiles/categories");
            match fs::read_to_string(path) {
                Ok(s) => s.filter_lines()
                    .filter_map(|(i, s)| match parse::category(s) {
                        Ok(_) => Some(s.to_string()),
                        Err(e) => {
                            warn!("{}::profiles/categories, line {}: {e}", self.id, i + 1);
                            None
                        }
                    })
                    .collect(),
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}::profiles/categories: {e}", self.id);
                    }
                    Default::default()
                }
            }
        })
    }

    /// Return a repo's globally defined mirrors.
    pub fn mirrors(&self) -> &IndexMap<String, IndexSet<String>> {
        self.mirrors.get_or_init(|| {
            let path = self.path.join("profiles/thirdpartymirrors");
            match fs::read_to_string(path) {
                Ok(s) => s.filter_lines()
                    .filter_map(|(i, s)| {
                        let vals: Vec<_> = s.split_whitespace().collect();
                        if vals.len() <= 1 {
                            warn!(
                                "{}::profiles/thirdpartymirrors, line {}: no mirrors listed",
                                self.id,
                                i + 1
                            );
                            None
                        } else {
                            let name = vals[0].to_string();
                            let mirrors = vals[1..].iter().map(|s| s.to_string()).collect();
                            Some((name, mirrors))
                        }
                    })
                    .collect(),
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}::profiles/thirdpartymirrors: {e}", self.id);
                    }
                    Default::default()
                }
            }
        })
    }

    /// Return a repo's globally deprecated packages.
    pub fn pkg_deprecated(&self) -> &IndexSet<Dep> {
        self.pkg_deprecated.get_or_init(|| {
            let path = self.path.join("profiles/package.deprecated");
            match fs::read_to_string(path) {
                Ok(s) => s.filter_lines()
                    .filter_map(|(i, s)| match self.eapi.dep(s) {
                        Ok(dep) => Some(dep),
                        Err(e) => {
                            warn!("{}::profiles/package.deprecated, line {}: {e}", self.id, i + 1);
                            None
                        }
                    })
                    .collect(),
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}::profiles/package.deprecated: {e}", self.id);
                    }
                    Default::default()
                }
            }
        })
    }

    /// Return a repo's globally masked packages.
    pub fn pkg_mask(&self) -> &IndexSet<Dep> {
        self.pkg_mask.get_or_init(|| {
            let path = self.path.join("profiles/package.mask");
            match fs::read_to_string(path) {
                Ok(s) => s.filter_lines()
                    .filter_map(|(i, s)| match self.eapi.dep(s) {
                        Ok(dep) => Some(dep),
                        Err(e) => {
                            warn!("{}::profiles/package.mask, line {}: {e}", self.id, i + 1);
                            None
                        }
                    })
                    .collect(),
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}::profiles/package.mask: {e}", self.id);
                    }
                    Default::default()
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use crate::eapi::EAPI_LATEST_OFFICIAL;
    use crate::macros::*;
    use crate::repo::ebuild_temp::Repo as TempRepo;
    use crate::test::{assert_ordered_eq, assert_unordered_eq};

    use super::*;

    #[test]
    fn test_config() {
        let t = TempRepo::new("test", None, None).unwrap();

        // empty config
        fs::write(t.path().join("metadata/layout.conf"), "").unwrap();
        let metadata = Metadata::new("test", t.path()).unwrap();
        assert!(metadata.config().is_empty());

        // invalid config
        fs::write(t.path().join("metadata/layout.conf"), "data").unwrap();
        assert!(Metadata::new("test", t.path()).is_err());
    }

    #[test]
    fn test_config_properties_and_restrict_allowed() {
        let t = TempRepo::new("test", None, None).unwrap();

        // empty
        let metadata = Metadata::new("test", t.path()).unwrap();
        assert!(metadata.config().properties_allowed().is_empty());
        assert!(metadata.config().restrict_allowed().is_empty());

        // existing
        let data = indoc::indoc! {r#"
            properties-allowed = interactive live
            restrict-allowed = fetch mirror
        "#};
        fs::write(t.path().join("metadata/layout.conf"), data).unwrap();
        let metadata = Metadata::new("test", t.path()).unwrap();
        assert_unordered_eq(metadata.config().properties_allowed(), ["live", "interactive"]);
        assert_unordered_eq(metadata.config().restrict_allowed(), ["fetch", "mirror"]);
    }

    #[test]
    fn test_arches() {
        let t = TempRepo::new("test", None, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new("test", t.path()).unwrap();
        assert!(metadata.arches().is_empty());

        // empty file
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "").unwrap();
        assert!(metadata.arches().is_empty());

        // multiple
        let data = indoc::indoc! {r#"
            amd64
            arm64
            amd64-linux
        "#};
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), data).unwrap();
        assert_ordered_eq(metadata.arches(), ["amd64", "arm64", "amd64-linux"]);
    }

    #[traced_test]
    #[test]
    fn test_arches_desc() {
        let t = TempRepo::new("test", None, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new("test", t.path()).unwrap();
        assert!(metadata.arches_desc().is_empty());

        // empty file
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "").unwrap();
        assert!(metadata.arches_desc().is_empty());

        // invalid line format
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64\narm64").unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "amd64 stable\narm64").unwrap();
        assert!(!metadata.arches_desc().is_empty());
        assert_logs_re!(format!(".+, line 2: invalid line format: .+$"));

        // unknown arch
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64").unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "arm64 stable").unwrap();
        assert!(metadata.arches_desc().is_empty());
        assert_logs_re!(format!(".+, line 1: unknown arch: arm64$"));

        // unknown status
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64").unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "amd64 test").unwrap();
        assert!(metadata.arches_desc().is_empty());
        assert_logs_re!(format!(".+, line 1: unknown status: test$"));

        // multiple with ignored 3rd column
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64\narm64\nppc64").unwrap();
        fs::write(
            metadata.path.join("profiles/arches.desc"),
            "amd64 stable\narm64 testing\nppc64 transitional 3rd-col",
        )
        .unwrap();
        assert_unordered_eq(&metadata.arches_desc()[&ArchStatus::Stable], ["amd64"]);
        assert_unordered_eq(&metadata.arches_desc()[&ArchStatus::Testing], ["arm64"]);
        assert_unordered_eq(&metadata.arches_desc()[&ArchStatus::Transitional], ["ppc64"]);
    }

    #[traced_test]
    #[test]
    fn test_categories() {
        let t = TempRepo::new("test", None, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new("test", t.path()).unwrap();
        assert!(metadata.categories().is_empty());

        // empty file
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/categories"), "").unwrap();
        assert!(metadata.categories().is_empty());

        // multiple with invalid entry
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/categories"), "cat\nc@t").unwrap();
        assert_ordered_eq(metadata.categories(), ["cat"]);
        assert_logs_re!(format!(".+, line 2: .* invalid category name: c@t$"));

        // multiple
        let data = indoc::indoc! {r#"
            cat1
            cat2
            cat-3
        "#};
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/categories"), data).unwrap();
        assert_ordered_eq(metadata.categories(), ["cat1", "cat2", "cat-3"]);
    }

    #[test]
    fn test_mirrors() {
        let t = TempRepo::new("test", None, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new("test", t.path()).unwrap();
        assert!(metadata.mirrors().is_empty());

        // empty file
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/thirdpartymirrors"), "").unwrap();
        assert!(metadata.mirrors().is_empty());

        // multiple with mixed whitespace
        let data = indoc::indoc! {r#"
            # comment 1
            mirror1 https://a/mirror/ https://another/mirror

            # comment 2
            mirror2	http://yet/another/mirror/
        "#};
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/thirdpartymirrors"), data).unwrap();
        assert_ordered_eq(
            metadata.mirrors().get("mirror1").unwrap(),
            ["https://a/mirror/", "https://another/mirror"],
        );
        assert_ordered_eq(
            metadata.mirrors().get("mirror2").unwrap(),
            ["http://yet/another/mirror/"],
        );
    }

    #[traced_test]
    #[test]
    fn test_pkg_deprecated() {
        let t = TempRepo::new("test", None, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new("test", t.path()).unwrap();
        assert!(metadata.pkg_deprecated().is_empty());

        // empty file
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.deprecated"), "").unwrap();
        assert!(metadata.pkg_deprecated().is_empty());

        // multiple with invalid dep for repo EAPI
        let data = indoc::indoc! {r#"
            # comment 1
            cat/pkg-a

            # comment 2
            another/pkg

            # invalid for repo EAPI
            cat/slotted:0
        "#};
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.deprecated"), data).unwrap();
        assert_ordered_eq(
            metadata.pkg_deprecated(),
            [&Dep::from_str("cat/pkg-a").unwrap(), &Dep::from_str("another/pkg").unwrap()],
        );
        assert_logs_re!(format!(".+, line 8: .* invalid dep: cat/slotted:0$"));

        // newer repo EAPI allows using newer dep format features
        let t = TempRepo::new("test", None, Some(&EAPI_LATEST_OFFICIAL)).unwrap();
        // multiple with invalid dep for repo EAPI
        let data = indoc::indoc! {r#"
            cat/slotted:0
            cat/subslot:0/1
        "#};
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.deprecated"), data).unwrap();
        assert_ordered_eq(
            metadata.pkg_deprecated(),
            [&Dep::from_str("cat/slotted:0").unwrap(), &Dep::from_str("cat/subslot:0/1").unwrap()],
        );
    }

    #[traced_test]
    #[test]
    fn test_pkg_mask() {
        let t = TempRepo::new("test", None, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new("test", t.path()).unwrap();
        assert!(metadata.pkg_mask().is_empty());

        // empty file
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.mask"), "").unwrap();
        assert!(metadata.pkg_mask().is_empty());

        // multiple with invalid dep for repo EAPI
        let data = indoc::indoc! {r#"
            # comment 1
            cat/pkg-a

            # comment 2
            another/pkg

            # invalid for repo EAPI
            cat/slotted:0
        "#};
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.mask"), data).unwrap();
        assert_ordered_eq(
            metadata.pkg_mask(),
            [&Dep::from_str("cat/pkg-a").unwrap(), &Dep::from_str("another/pkg").unwrap()],
        );
        assert_logs_re!(format!(".+, line 8: .* invalid dep: cat/slotted:0$"));

        // newer repo EAPI allows using newer dep format features
        let t = TempRepo::new("test", None, Some(&EAPI_LATEST_OFFICIAL)).unwrap();
        // multiple with invalid dep for repo EAPI
        let data = indoc::indoc! {r#"
            cat/slotted:0
            cat/subslot:0/1
        "#};
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.mask"), data).unwrap();
        assert_ordered_eq(
            metadata.pkg_mask(),
            [&Dep::from_str("cat/slotted:0").unwrap(), &Dep::from_str("cat/subslot:0/1").unwrap()],
        );
    }
}
