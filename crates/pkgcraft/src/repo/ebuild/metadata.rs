use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::str::{FromStr, SplitWhitespace};
use std::sync::OnceLock;
use std::{fmt, fs, io};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{IndexMap, IndexSet};
use strum::{Display, EnumString};
use tracing::{error, warn};

use crate::dep::{parse, Dep};
use crate::eapi::Eapi;
use crate::files::{is_file, is_hidden, sorted_dir_list};
use crate::pkg::ebuild::metadata::HashType;
use crate::repo::RepoFormat;
use crate::traits::FilterLines;
use crate::types::{OrderedMap, OrderedSet};
use crate::Error;

use super::cache::CacheFormat;

/// Wrapper for ini format config files.
struct Ini(ini::Ini);

impl fmt::Debug for Ini {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let section = self.0.general_section();
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
            .general_section()
            .get(key)
            .unwrap_or_default()
            .split_whitespace()
    }
}

/// Ebuild repo configuration as defined by GLEP 82.
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Config {
    cache_formats: OrderedSet<CacheFormat>,
    manifest_hashes: OrderedSet<HashType>,
    manifest_required_hashes: OrderedSet<HashType>,
    masters: OrderedSet<String>,
    properties_allowed: OrderedSet<String>,
    restrict_allowed: OrderedSet<String>,
}

macro_rules! ordered_set {
    ($ini:expr, $key:expr, $type:ident) => {
        $ini.iter($key)
            .map(|s| {
                $type::from_str(s)
                    .map_err(|_| Error::InvalidValue(format!("unsupported {}: {s}", $key)))
            })
            .collect::<crate::Result<OrderedSet<_>>>()
    };
}

impl Config {
    fn new(repo_path: &Utf8Path) -> crate::Result<Self> {
        let path = repo_path.join("metadata/layout.conf");
        let ini = Ini::load(&path)?;

        Ok(Self {
            cache_formats: ordered_set!(ini, "cache-formats", CacheFormat)?,
            manifest_hashes: ordered_set!(ini, "manifest-hashes", HashType)?,
            manifest_required_hashes: ordered_set!(ini, "manifest-required-hashes", HashType)?,
            masters: ordered_set!(ini, "masters", String)?,
            properties_allowed: ordered_set!(ini, "properties-allowed", String)?,
            restrict_allowed: ordered_set!(ini, "restrict-allowed", String)?,
        })
    }

    /// Return the ordered set of metadata cache types.
    pub fn cache_formats(&self) -> &OrderedSet<CacheFormat> {
        &self.cache_formats
    }

    /// Return the ordered set of hash types that must be used for Manifest entries.
    pub fn manifest_required_hashes(&self) -> &OrderedSet<HashType> {
        &self.manifest_required_hashes
    }

    /// Return the ordered set of hash types that should be used for Manifest entries.
    pub fn manifest_hashes(&self) -> &OrderedSet<HashType> {
        &self.manifest_hashes
    }

    /// Return the ordered set of inherited repo ids.
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
                let d1 = Dep::new_cpn(s1)?;
                let d2 = Dep::new_cpn(s2)?;
                Ok(Self::Move(d1, d2))
            }
            ["slotmove", spec, s1, s2] => {
                let dep = Dep::from_str(spec)?;
                let s1 = parse::slot(s1)?;
                let s2 = parse::slot(s2)?;
                Ok(Self::SlotMove(dep, s1.to_string(), s2.to_string()))
            }
            _ => Err(Error::InvalidValue(format!("invalid or unknown update: {s}"))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UseDesc {
    name: String,
    desc: String,
}

impl UseDesc {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn desc(&self) -> &str {
        &self.desc
    }
}

impl Ord for UseDesc {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for UseDesc {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for UseDesc {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for UseDesc {}

impl Hash for UseDesc {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl UseDesc {
    fn new(name: &str, desc: &str) -> crate::Result<Self> {
        Ok(Self {
            name: parse::use_flag(name).map(|s| s.to_string())?,
            desc: desc.to_string(),
        })
    }
}

impl FromStr for UseDesc {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let (flag, desc) = s
            .split_once(" - ")
            .ok_or_else(|| Error::InvalidValue(s.to_string()))?;
        UseDesc::new(flag, desc)
    }
}

#[derive(Debug, Default)]
pub struct Metadata {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) eapi: &'static Eapi,
    config: Config,
    path: Utf8PathBuf,
    cache_path: Utf8PathBuf,
    arches: OnceLock<IndexSet<String>>,
    arches_desc: OnceLock<HashMap<ArchStatus, HashSet<String>>>,
    categories: OnceLock<IndexSet<String>>,
    licenses: OnceLock<IndexSet<String>>,
    license_groups: OnceLock<HashMap<String, HashSet<String>>>,
    mirrors: OnceLock<IndexMap<String, IndexSet<String>>>,
    pkg_deprecated: OnceLock<IndexSet<Dep>>,
    pkg_mask: OnceLock<IndexSet<Dep>>,
    updates: OnceLock<IndexSet<PkgUpdate>>,
    use_desc: OnceLock<IndexSet<UseDesc>>,
    use_expand_desc: OnceLock<IndexMap<String, IndexSet<UseDesc>>>,
    use_local_desc: OnceLock<OrderedMap<String, OrderedSet<UseDesc>>>,
}

impl Metadata {
    pub(super) fn new(id: &str, path: &Utf8Path) -> crate::Result<Self> {
        let not_a_repo = |err: String| -> Error {
            Error::NotARepo {
                kind: RepoFormat::Ebuild,
                id: id.to_string(),
                err,
            }
        };
        let invalid_repo =
            |err: String| -> Error { Error::InvalidRepo { id: id.to_string(), err } };

        // verify repo name
        let name = match fs::read_to_string(path.join("profiles/repo_name")) {
            Ok(data) => match data.lines().next().map(|s| parse::repo(s.trim())) {
                Some(Ok(s)) => Ok(s.to_string()),
                Some(Err(e)) => Err(invalid_repo(format!("profiles/repo_name: {e}"))),
                None => Err(invalid_repo("profiles/repo_name empty".to_string())),
            },
            Err(e) => Err(not_a_repo(format!("profiles/repo_name: {e}"))),
        }?;

        // verify repo EAPI
        let eapi = path
            .join("profiles/eapi")
            .as_path()
            .try_into()
            .map_err(|e| invalid_repo(format!("profiles/eapi: {e}")))?;

        let config =
            Config::new(path).map_err(|e| invalid_repo(format!("metadata/layout.conf: {e}")))?;

        Ok(Self {
            id: id.to_string(),
            name,
            eapi,
            config,
            path: Utf8PathBuf::from(path),
            cache_path: path.join("metadata/md5-cache"),
            ..Default::default()
        })
    }

    pub fn cache_path(&self) -> &Utf8Path {
        &self.cache_path
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
                    let mut vals: IndexSet<_> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string())
                        .collect();
                    vals.sort();
                    vals
                }
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}: reading licenses failed: {e}", self.id);
                    }
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

    /// Return the ordered map of global USE flags.
    pub fn use_desc(&self) -> &IndexSet<UseDesc> {
        self.use_desc.get_or_init(|| {
            self.read_path("profiles/use.desc")
                .filter_lines()
                .filter_map(|(i, s)| {
                    UseDesc::from_str(s)
                        .map_err(|e| {
                            warn!("{}::profiles/use.desc, line {i}: invalid format: {e}", self.id);
                        })
                        .ok()
                })
                .collect()
        })
    }

    /// Return the ordered map of USE_EXPAND flags.
    pub fn use_expand_desc(&self) -> &IndexMap<String, IndexSet<UseDesc>> {
        self.use_expand_desc.get_or_init(|| {
            sorted_dir_list(self.path.join("profiles/desc"))
                .into_iter()
                .filter_entry(|e| is_file(e) && !is_hidden(e))
                .filter_map(|e| e.ok())
                .filter_map(|e| fs::read_to_string(e.path()).ok().map(|s| (e, s)))
                .map(|(e, s)| {
                    let file = e.file_name().to_str().unwrap_or_default();
                    let name = e
                        .path()
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default();
                    let vals = s
                        .filter_lines()
                        .filter_map(|(i, line)| {
                            UseDesc::from_str(line)
                                .map_err(|err| {
                                    warn!("{}::profiles/desc/{file}, line {i}: {err}", self.id)
                                })
                                .ok()
                        })
                        .collect::<IndexSet<_>>();
                    (name.to_string(), vals)
                })
                .collect()
        })
    }

    /// Return the ordered map of local USE flags.
    pub fn use_local_desc(&self) -> &OrderedMap<String, OrderedSet<UseDesc>> {
        // parse a use.local.desc line
        let parse = |s: &str| -> crate::Result<(String, UseDesc)> {
            let (cpn, use_desc) = s
                .split_once(':')
                .ok_or_else(|| Error::InvalidValue(s.to_string()))?;
            let dep = Dep::new_cpn(cpn)?;
            Ok((dep.to_string(), UseDesc::from_str(use_desc)?))
        };

        self.use_local_desc.get_or_init(|| {
            self.read_path("profiles/use.local.desc")
                .filter_lines()
                .filter_map(|(i, s)| {
                    parse(s)
                        .map_err(|e| {
                            warn!(
                                "{}::profiles/use.local.desc, line {i}: invalid format: {e}",
                                self.id
                            );
                        })
                        .ok()
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
    use crate::test::{assert_ordered_eq, assert_unordered_eq};

    use super::*;

    #[test]
    fn test_config() {
        let mut config = crate::config::Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();

        // empty config
        fs::write(repo.path().join("metadata/layout.conf"), "").unwrap();
        let metadata = Metadata::new("test", repo.path()).unwrap();
        assert!(metadata.config().is_empty());

        // invalid config
        fs::write(repo.path().join("metadata/layout.conf"), "data").unwrap();
        assert!(Metadata::new("test", repo.path()).is_err());
    }

    #[test]
    fn test_config_properties_and_restrict_allowed() {
        let mut config = crate::config::Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();

        // empty
        let metadata = Metadata::new("test", repo.path()).unwrap();
        assert!(metadata.config().properties_allowed().is_empty());
        assert!(metadata.config().restrict_allowed().is_empty());

        // existing
        let data = indoc::indoc! {r#"
            properties-allowed = interactive live
            restrict-allowed = fetch mirror
        "#};
        fs::write(repo.path().join("metadata/layout.conf"), data).unwrap();
        let metadata = Metadata::new("test", repo.path()).unwrap();
        assert_unordered_eq(metadata.config().properties_allowed(), ["live", "interactive"]);
        assert_unordered_eq(metadata.config().restrict_allowed(), ["fetch", "mirror"]);
    }

    #[test]
    fn test_arches() {
        let mut config = crate::config::Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new("test", repo.path()).unwrap();
        assert!(metadata.arches().is_empty());

        // empty file
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "").unwrap();
        assert!(metadata.arches().is_empty());

        // multiple
        let data = indoc::indoc! {r#"
            amd64
            arm64
            amd64-linux
        "#};
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), data).unwrap();
        assert_ordered_eq(metadata.arches(), ["amd64", "arm64", "amd64-linux"]);
    }

    #[traced_test]
    #[test]
    fn test_arches_desc() {
        let mut config = crate::config::Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new("test", repo.path()).unwrap();
        assert!(metadata.arches_desc().is_empty());

        // empty file
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "").unwrap();
        assert!(metadata.arches_desc().is_empty());

        // invalid line format
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64\narm64").unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "amd64 stable\narm64").unwrap();
        assert!(!metadata.arches_desc().is_empty());
        assert_logs_re!(".+, line 2: invalid line format: .+$");

        // unknown arch
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64").unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "arm64 stable").unwrap();
        assert!(metadata.arches_desc().is_empty());
        assert_logs_re!(".+, line 1: unknown arch: arm64$");

        // unknown status
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64").unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "amd64 test").unwrap();
        assert!(metadata.arches_desc().is_empty());
        assert_logs_re!(".+, line 1: unknown status: test$");

        // multiple with ignored 3rd column
        let metadata = Metadata::new("test", repo.path()).unwrap();
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
        let mut config = crate::config::Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new("test", repo.path()).unwrap();
        assert!(metadata.categories().is_empty());

        // empty file
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/categories"), "").unwrap();
        assert!(metadata.categories().is_empty());

        // multiple with invalid entry
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/categories"), "cat\nc@t").unwrap();
        assert_ordered_eq(metadata.categories(), ["cat"]);
        assert_logs_re!(".+, line 2: .* invalid category name: c@t$");

        // multiple
        let data = indoc::indoc! {r#"
            cat1
            cat2
            cat-3
        "#};
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/categories"), data).unwrap();
        assert_ordered_eq(metadata.categories(), ["cat1", "cat2", "cat-3"]);
    }

    #[test]
    fn test_licenses() {
        let mut config = crate::config::Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();

        // nonexistent dir
        let metadata = Metadata::new("test", repo.path()).unwrap();
        assert!(metadata.licenses().is_empty());

        // empty dir
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::create_dir(metadata.path.join("licenses")).unwrap();
        assert!(metadata.licenses().is_empty());

        // multiple
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("licenses/L1"), "").unwrap();
        fs::write(metadata.path.join("licenses/L2"), "").unwrap();
        assert_ordered_eq(metadata.licenses(), ["L1", "L2"]);
    }

    #[traced_test]
    #[test]
    fn test_license_groups() {
        let mut config = crate::config::Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();

        // nonexistent dir
        let metadata = Metadata::new("test", repo.path()).unwrap();
        assert!(metadata.license_groups().is_empty());

        // empty file
        let metadata = Metadata::new("test", repo.path()).unwrap();
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
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/license_groups"), data).unwrap();
        assert_unordered_eq(metadata.license_groups().get("set1").unwrap(), ["a", "b"]);
        assert_unordered_eq(metadata.license_groups().get("set2").unwrap(), ["a"]);
        assert_logs_re!(".+, line 5: unknown license: z$");

        // multiple with unknown and invalid aliases
        let data = indoc::indoc! {r#"
            # comment 1
            set1 b @

            # comment 2
            set2 a c @set1 @set3
        "#};
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/license_groups"), data).unwrap();
        assert_unordered_eq(metadata.license_groups().get("set1").unwrap(), ["b"]);
        assert_unordered_eq(metadata.license_groups().get("set2").unwrap(), ["a", "b", "c"]);
        assert_logs_re!(".+, line 2: invalid alias: @");
        assert_logs_re!(".+ set2: unknown alias: set3");

        // multiple with cyclic aliases
        let data = indoc::indoc! {r#"
            set1 a @set2
            set2 b @set1
            set3 c @set2
            set4 c @set4
        "#};
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/license_groups"), data).unwrap();
        assert_unordered_eq(metadata.license_groups().get("set1").unwrap(), ["a", "b"]);
        assert_unordered_eq(metadata.license_groups().get("set2").unwrap(), ["a", "b"]);
        assert_unordered_eq(metadata.license_groups().get("set3").unwrap(), ["a", "b", "c"]);
        assert_unordered_eq(metadata.license_groups().get("set4").unwrap(), ["c"]);
        assert_logs_re!(".+ set1: cyclic alias: set2");
        assert_logs_re!(".+ set2: cyclic alias: set1");
        assert_logs_re!(".+ set3: cyclic alias: set2");
        assert_logs_re!(".+ set4: cyclic alias: set4");
    }

    #[test]
    fn test_mirrors() {
        let mut config = crate::config::Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new("test", repo.path()).unwrap();
        assert!(metadata.mirrors().is_empty());

        // empty file
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/thirdpartymirrors"), "").unwrap();
        assert!(metadata.mirrors().is_empty());

        // multiple with mixed whitespace
        let data = indoc::indoc! {r#"
            # comment 1
            mirror1 https://a/mirror/ https://another/mirror

            # comment 2
            mirror2	http://yet/another/mirror/
        "#};
        let metadata = Metadata::new("test", repo.path()).unwrap();
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
        let mut config = crate::config::Config::default();
        let repo = config.temp_repo("test1", 0, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new("test", repo.path()).unwrap();
        assert!(metadata.pkg_deprecated().is_empty());

        // empty file
        let metadata = Metadata::new("test", repo.path()).unwrap();
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
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.deprecated"), data).unwrap();
        assert_ordered_eq(
            metadata.pkg_deprecated(),
            [&Dep::from_str("cat/pkg-a").unwrap(), &Dep::from_str("another/pkg").unwrap()],
        );
        assert_logs_re!(".+, line 8: .* invalid dep: cat/slotted:0$");

        // newer repo EAPI allows using newer dep format features
        let repo = config
            .temp_repo("test2", 0, Some(&EAPI_LATEST_OFFICIAL))
            .unwrap();
        // multiple with invalid dep for repo EAPI
        let data = indoc::indoc! {r#"
            cat/slotted:0
            cat/subslot:0/1
        "#};
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.deprecated"), data).unwrap();
        assert_ordered_eq(
            metadata.pkg_deprecated(),
            [&Dep::from_str("cat/slotted:0").unwrap(), &Dep::from_str("cat/subslot:0/1").unwrap()],
        );
    }

    #[traced_test]
    #[test]
    fn test_pkg_mask() {
        let mut config = crate::config::Config::default();
        let repo = config.temp_repo("test1", 0, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new("test", repo.path()).unwrap();
        assert!(metadata.pkg_mask().is_empty());

        // empty file
        let metadata = Metadata::new("test", repo.path()).unwrap();
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
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.mask"), data).unwrap();
        assert_ordered_eq(
            metadata.pkg_mask(),
            [&Dep::from_str("cat/pkg-a").unwrap(), &Dep::from_str("another/pkg").unwrap()],
        );
        assert_logs_re!(".+, line 8: .* invalid dep: cat/slotted:0$");

        // newer repo EAPI allows using newer dep format features
        let repo = config
            .temp_repo("test2", 0, Some(&EAPI_LATEST_OFFICIAL))
            .unwrap();
        // multiple with invalid dep for repo EAPI
        let data = indoc::indoc! {r#"
            cat/slotted:0
            cat/subslot:0/1
        "#};
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.mask"), data).unwrap();
        assert_ordered_eq(
            metadata.pkg_mask(),
            [&Dep::from_str("cat/slotted:0").unwrap(), &Dep::from_str("cat/subslot:0/1").unwrap()],
        );
    }

    #[traced_test]
    #[test]
    fn test_updates() {
        let mut config = crate::config::Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();

        // nonexistent
        let metadata = Metadata::new("test", repo.path()).unwrap();
        assert!(metadata.updates().is_empty());

        // empty
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::create_dir_all(metadata.path.join("profiles/updates")).unwrap();
        fs::write(metadata.path.join("profiles/updates/1Q-9999"), "").unwrap();
        assert!(metadata.updates().is_empty());

        // multiple with invalid
        let data = indoc::indoc! {r#"
            # valid move
            move cat/pkg1 cat/pkg2

            # invalid cpn
            move cat/pkg3-1 cat/pkg4

            # valid slotmove
            slotmove <cat/pkg1-5 0 1

            # invalid slot
            slotmove >cat/pkg1-5 @ 1
        "#};
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/updates/1Q-9999"), data).unwrap();
        let updates = metadata.updates();
        assert_eq!(updates.len(), 2);
        assert_logs_re!(".+ line 5: .+?: invalid unversioned dep: cat/pkg3-1$");
        assert_logs_re!(".+ line 11: .+?: invalid slot: @$");
    }

    #[traced_test]
    #[test]
    fn test_use_desc() {
        let mut config = crate::config::Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();

        // nonexistent
        let metadata = Metadata::new("test", repo.path()).unwrap();
        assert!(metadata.use_desc().is_empty());

        // empty
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/use.desc"), "").unwrap();
        assert!(metadata.use_desc().is_empty());

        // multiple with invalid
        let data = indoc::indoc! {r#"
            # normal
            a - a flag description

            # invalid format
            b: b flag description

            # invalid USE flag
            @c - c flag description
        "#};
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/use.desc"), data).unwrap();
        assert_ordered_eq(metadata.use_desc(), [&UseDesc::new("a", "a flag description").unwrap()]);
        assert_logs_re!(".+ line 5: invalid format: b: b flag description$");
        assert_logs_re!(".+ line 8: .+?: invalid USE flag: @c$");
    }

    #[traced_test]
    #[test]
    fn test_use_local_desc() {
        let mut config = crate::config::Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();

        // nonexistent
        let metadata = Metadata::new("test", repo.path()).unwrap();
        assert!(metadata.use_local_desc().is_empty());

        // empty
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/use.local.desc"), "").unwrap();
        assert!(metadata.use_local_desc().is_empty());

        // multiple with invalid
        let data = indoc::indoc! {r#"
            # normal
            cat/pkg:a - a flag description

            # invalid format
            b - b flag description

            # invalid USE flag
            cat/pkg:@c - c flag description
        "#};
        let metadata = Metadata::new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/use.local.desc"), data).unwrap();
        assert_ordered_eq(
            metadata.use_local_desc().get("cat/pkg").unwrap(),
            [&UseDesc::new("a", "a flag description").unwrap()],
        );
        assert_logs_re!(".+ line 5: invalid format: b - b flag description$");
        assert_logs_re!(".+ line 8: .+?: invalid USE flag: @c$");
    }
}
