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
use crate::files::{is_file, is_hidden, sorted_dir_list};
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
            .map(|(i, s)| (i + 1, s))
            .filter(|(_, s)| !s.is_empty() && !s.starts_with('#'));

        Box::new(iter)
    }
}

trait FileReader {
    fn read_path(&self, relpath: &str) -> String;
}

impl FileReader for Metadata {
    fn read_path(&self, relpath: &str) -> String {
        let path = self.path.join(relpath);
        fs::read_to_string(path).unwrap_or_else(|e| {
            if e.kind() != io::ErrorKind::NotFound {
                warn!("{}::{relpath}: {e}", self.id);
            }
            Default::default()
        })
    }
}

#[derive(Display, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum ArchStatus {
    Stable,
    Testing,
    Transitional,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum PkgUpdate {
    Move(Dep, Dep),
    SlotMove(Dep, String, String),
}

impl FromStr for PkgUpdate {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let tokens: Vec<_> = s.split_whitespace().collect();
        match &tokens[..] {
            ["move", s1, s2] => {
                let d1 = Dep::unversioned(s1)?;
                let d2 = Dep::unversioned(s2)?;
                Ok(Self::Move(d1, d2))
            }
            ["slotmove", spec, s1, s2] => {
                let dep = Dep::from_str(spec)?;
                // TODO: validate slot names
                Ok(Self::SlotMove(dep, s1.to_string(), s2.to_string()))
            }
            _ => Err(Error::InvalidValue(format!("invalid or unknown update: {s}"))),
        }
    }
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
    licenses: OnceCell<IndexSet<String>>,
    license_groups: OnceCell<HashMap<String, HashSet<String>>>,
    mirrors: OnceCell<IndexMap<String, IndexSet<String>>>,
    pkg_deprecated: OnceCell<IndexSet<Dep>>,
    pkg_mask: OnceCell<IndexSet<Dep>>,
    updates: OnceCell<IndexSet<PkgUpdate>>,
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
            self.read_path("profiles/arch.list")
                .filter_lines()
                .map(|(_, s)| s.to_string())
                .collect()
        })
    }

    /// Architecture stability status from `profiles/arches.desc`.
    /// See GLEP 72 (https://www.gentoo.org/glep/glep-0072.html).
    pub fn arches_desc(&self) -> &HashMap<ArchStatus, HashSet<String>> {
        self.arches_desc.get_or_init(|| {
            let mut vals = HashMap::<ArchStatus, HashSet<String>>::new();
            self.read_path("profiles/arches.desc")
                .filter_lines()
                .map(|(i, s)| (i, s.split_whitespace()))
                // only pull the first two columns, ignoring any additional
                .for_each(|(i, mut iter)| match (iter.next(), iter.next()) {
                    (Some(arch), Some(status)) => {
                        if !self.arches().contains(arch) {
                            warn!(
                                "{}::profiles/arches.desc, line {i}: unknown arch: {arch}",
                                self.id
                            );
                            return;
                        }

                        if let Ok(status) = ArchStatus::from_str(status) {
                            vals.entry(status)
                                .or_insert_with(HashSet::new)
                                .insert(arch.to_string());
                        } else {
                            warn!(
                                "{}::profiles/arches.desc, line {i}: unknown status: {status}",
                                self.id,
                            );
                        }
                    }
                    _ => error!(
                        "{}::profiles/arches.desc, line {i}: \
                        invalid line format: should be '<arch> <status>'",
                        self.id,
                    ),
                });

            vals
        })
    }

    /// Return a repo's configured categories from `profiles/categories`.
    pub fn categories(&self) -> &IndexSet<String> {
        self.categories.get_or_init(|| {
            self.read_path("profiles/categories")
                .filter_lines()
                .filter_map(|(i, s)| match parse::category(s) {
                    Ok(_) => Some(s.to_string()),
                    Err(e) => {
                        warn!("{}::profiles/categories, line {i}: {e}", self.id);
                        None
                    }
                })
                .collect()
        })
    }

    /// Return the ordered set of licenses.
    pub fn licenses(&self) -> &IndexSet<String> {
        self.licenses
            .get_or_init(|| match self.path.join("licenses").read_dir_utf8() {
                Ok(entries) => {
                    let mut vals: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string())
                        .collect();
                    vals.sort();
                    vals.into_iter().collect()
                }
                Err(e) => {
                    warn!("{}: reading licenses failed: {e}", self.id);
                    Default::default()
                }
            })
    }

    /// Return the mapping of license groups.
    pub fn license_groups(&self) -> &HashMap<String, HashSet<String>> {
        self.license_groups.get_or_init(|| {
            let mut alias_map = IndexMap::<String, IndexSet<String>>::new();
            let mut group_map: HashMap<_, _> = self.read_path("profiles/license_groups")
                .filter_lines()
                .filter_map(|(i, s)| {
                    let mut vals = s.split_whitespace();
                    if let Some(name) = vals.next() {
                        let licenses = vals
                            .filter_map(|s| match s.strip_prefix('@') {
                                None => {
                                    if self.licenses().contains(s) {
                                        Some(s.to_string())
                                    } else {
                                        warn!(
                                            "{}::profiles/license_groups, line {i}: unknown license: {s}",
                                            self.id,
                                        );
                                        None
                                    }
                                }
                                Some(alias) => {
                                    if !alias.is_empty() {
                                        alias_map.entry(name.to_string())
                                            .or_insert_with(IndexSet::new)
                                            .insert(alias.to_string());
                                    } else {
                                        warn!(
                                            "{}::profiles/license_groups, line {i}: invalid alias: {s}",
                                            self.id,
                                        );
                                    }
                                    None
                                }
                            })
                            .collect();
                        Some((name.to_string(), licenses))
                    } else {
                        None
                    }
                })
                .collect();

            // resolve aliases using DFS
            for (set, aliases) in &alias_map {
                let mut seen = HashSet::new();
                let mut stack = aliases.clone();
                while let Some(s) = stack.pop() {
                    if !seen.contains(&s) {
                        seen.insert(s.clone());
                    }

                    // push unresolved, nested aliases onto the stack
                    if let Some(nested_aliases) = alias_map.get(&s) {
                        for x in nested_aliases {
                            if !seen.contains(x) {
                                stack.insert(x.clone());
                            } else {
                                warn!(
                                    "{}::profiles/license_groups: {set}: cyclic alias: {x}",
                                    self.id,
                                );
                            }
                        }
                    }

                    // resolve alias values
                    if let Some(values) = group_map.get(&s).cloned() {
                        group_map.entry(set.clone())
                            .or_insert_with(HashSet::new)
                            .extend(values);
                    } else {
                        warn!(
                            "{}::profiles/license_groups: {set}: unknown alias: {s}",
                            self.id,
                        );
                    }
                }
            }

            group_map
        })
    }

    /// Return a repo's globally defined mirrors.
    pub fn mirrors(&self) -> &IndexMap<String, IndexSet<String>> {
        self.mirrors.get_or_init(|| {
            self.read_path("profiles/thirdpartymirrors")
                .filter_lines()
                .filter_map(|(i, s)| {
                    let vals: Vec<_> = s.split_whitespace().collect();
                    if vals.len() <= 1 {
                        warn!(
                            "{}::profiles/thirdpartymirrors, line {i}: no mirrors listed",
                            self.id,
                        );
                        None
                    } else {
                        let name = vals[0].to_string();
                        let mirrors = vals[1..].iter().map(|s| s.to_string()).collect();
                        Some((name, mirrors))
                    }
                })
                .collect()
        })
    }

    /// Return a repo's globally deprecated packages.
    pub fn pkg_deprecated(&self) -> &IndexSet<Dep> {
        self.pkg_deprecated.get_or_init(|| {
            self.read_path("profiles/package.deprecated")
                .filter_lines()
                .filter_map(|(i, s)| match self.eapi.dep(s) {
                    Ok(dep) => Some(dep),
                    Err(e) => {
                        warn!("{}::profiles/package.deprecated, line {i}: {e}", self.id);
                        None
                    }
                })
                .collect()
        })
    }

    /// Return a repo's globally masked packages.
    pub fn pkg_mask(&self) -> &IndexSet<Dep> {
        self.pkg_mask.get_or_init(|| {
            self.read_path("profiles/package.mask")
                .filter_lines()
                .filter_map(|(i, s)| match self.eapi.dep(s) {
                    Ok(dep) => Some(dep),
                    Err(e) => {
                        warn!("{}::profiles/package.mask, line {i}: {e}", self.id);
                        None
                    }
                })
                .collect()
        })
    }

    /// Return the ordered set of package updates.
    pub fn updates(&self) -> &IndexSet<PkgUpdate> {
        self.updates.get_or_init(|| {
            sorted_dir_list(self.path.join("profiles/updates"))
                .into_iter()
                .filter_entry(|e| is_file(e) && !is_hidden(e))
                .filter_map(|e| e.ok())
                .filter_map(|e| fs::read_to_string(e.path()).ok().map(|s| (e, s)))
                .flat_map(|(e, s)| {
                    let file = e.file_name().to_str().unwrap_or_default();
                    // TODO: Note that comments and empty lines are filtered even though
                    // the specification doesn't allow them.
                    s.filter_lines()
                        .filter_map(|(i, line)| {
                            PkgUpdate::from_str(line)
                                .map_err(|err| {
                                    warn!("{}::profiles/updates/{file}, line {i}: {err}", self.id)
                                })
                                .ok()
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
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
    fn test_licenses() {
        let t = TempRepo::new("test", None, None).unwrap();

        // nonexistent dir
        let metadata = Metadata::new("test", t.path()).unwrap();
        assert!(metadata.licenses().is_empty());

        // empty dir
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::create_dir(metadata.path.join("licenses")).unwrap();
        assert!(metadata.licenses().is_empty());

        // multiple
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("licenses/L1"), "").unwrap();
        fs::write(metadata.path.join("licenses/L2"), "").unwrap();
        assert_ordered_eq(metadata.licenses(), ["L1", "L2"]);
    }

    #[traced_test]
    #[test]
    fn test_license_groups() {
        let t = TempRepo::new("test", None, None).unwrap();

        // nonexistent dir
        let metadata = Metadata::new("test", t.path()).unwrap();
        assert!(metadata.license_groups().is_empty());

        // empty file
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/license_groups"), "").unwrap();
        assert!(metadata.license_groups().is_empty());

        // create license files
        fs::create_dir(metadata.path.join("licenses")).unwrap();
        for l in ["a", "b", "c"] {
            fs::write(metadata.path.join(format!("licenses/{l}")), "").unwrap();
        }

        // multiple with unknown and mixed whitespace
        let data = indoc::indoc! {r#"
            # comment 1
            set1 a b

            # comment 2
            set2 a	z
        "#};
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/license_groups"), data).unwrap();
        assert_unordered_eq(metadata.license_groups().get("set1").unwrap(), ["a", "b"]);
        assert_unordered_eq(metadata.license_groups().get("set2").unwrap(), ["a"]);
        assert_logs_re!(format!(".+, line 5: unknown license: z$"));

        // multiple with unknown and invalid aliases
        let data = indoc::indoc! {r#"
            # comment 1
            set1 b @

            # comment 2
            set2 a c @set1 @set3
        "#};
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/license_groups"), data).unwrap();
        assert_unordered_eq(metadata.license_groups().get("set1").unwrap(), ["b"]);
        assert_unordered_eq(metadata.license_groups().get("set2").unwrap(), ["a", "b", "c"]);
        assert_logs_re!(format!(".+, line 2: invalid alias: @"));
        assert_logs_re!(format!(".+ set2: unknown alias: set3"));

        // multiple with cyclic aliases
        let data = indoc::indoc! {r#"
            set1 a @set2
            set2 b @set1
            set3 c @set2
            set4 c @set4
        "#};
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/license_groups"), data).unwrap();
        assert_unordered_eq(metadata.license_groups().get("set1").unwrap(), ["a", "b"]);
        assert_unordered_eq(metadata.license_groups().get("set2").unwrap(), ["a", "b"]);
        assert_unordered_eq(metadata.license_groups().get("set3").unwrap(), ["a", "b", "c"]);
        assert_unordered_eq(metadata.license_groups().get("set4").unwrap(), ["c"]);
        assert_logs_re!(format!(".+ set1: cyclic alias: set2"));
        assert_logs_re!(format!(".+ set2: cyclic alias: set1"));
        assert_logs_re!(format!(".+ set3: cyclic alias: set2"));
        assert_logs_re!(format!(".+ set4: cyclic alias: set4"));
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

    #[traced_test]
    #[test]
    fn test_updates() {
        let t = TempRepo::new("test", None, None).unwrap();

        // nonexistent
        let metadata = Metadata::new("test", t.path()).unwrap();
        assert!(metadata.updates().is_empty());

        // empty
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::create_dir_all(metadata.path.join("profiles/updates")).unwrap();
        fs::write(metadata.path.join("profiles/updates/1Q-9999"), "").unwrap();
        assert!(metadata.updates().is_empty());

        // multiple with invalid
        let data = indoc::indoc! {r#"
            # comment 1
            move cat/pkg1 cat/pkg2

            # invalid
            move cat/pkg3-1 cat/pkg4

            # comment 2
            slotmove <cat/pkg1-5 0 1
        "#};
        let metadata = Metadata::new("test", t.path()).unwrap();
        fs::write(metadata.path.join("profiles/updates/1Q-9999"), data).unwrap();
        let updates = metadata.updates();
        assert_eq!(updates.len(), 2);
        assert_logs_re!(format!(".+: invalid unversioned dep: cat/pkg3-1$"));
    }
}
