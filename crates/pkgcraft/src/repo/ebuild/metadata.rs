use std::borrow::Borrow;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
use std::{fs, io};

use camino::{Utf8DirEntry, Utf8Path, Utf8PathBuf};
use dashmap::DashMap;
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};
use tracing::{error, warn};
use url::Url;

use crate::Error;
use crate::dep::{Cpn, Dep, parse};
use crate::eapi::Eapi;
use crate::files::{
    atomic_write_file, has_ext_utf8, is_file, is_file_utf8, is_hidden, is_hidden_utf8,
    sorted_dir_list,
};
use crate::macros::build_path;
use crate::pkg::ebuild::keyword::Arch;
use crate::pkg::ebuild::manifest::Manifest;
use crate::pkg::ebuild::xml;
use crate::repo::{PkgRepository, RepoFormat};
use crate::traits::FilterLines;
use crate::types::OrderedMap;

use super::Eclass;
use super::cache::{CacheFormat, MetadataCache};

mod config;
use config::Config;

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

#[derive(AsRefStr, Display, EnumString, Debug, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum ArchStatus {
    Stable,
    Testing,
    Transitional,
}

impl PartialEq for ArchStatus {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl Eq for ArchStatus {}

impl Hash for ArchStatus {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

impl Borrow<str> for ArchStatus {
    fn borrow(&self) -> &str {
        self.as_ref()
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum PkgUpdate {
    Move(Cpn, Cpn),
    SlotMove(Dep, String, String),
}

impl FromStr for PkgUpdate {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let tokens: Vec<_> = s.split_whitespace().collect();
        match &tokens[..] {
            ["move", s1, s2] => Ok(Self::Move(s1.parse()?, s2.parse()?)),
            ["slotmove", dep, s1, s2] => {
                let s1 = parse::slot(s1)?;
                let s2 = parse::slot(s2)?;
                Ok(Self::SlotMove(dep.parse()?, s1.to_string(), s2.to_string()))
            }
            _ => Err(Error::InvalidValue(format!("invalid or unknown update: {s}"))),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Mirror {
    name: String,
    url: Url,
}

impl Mirror {
    /// Return the mirror's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return a mirrored url for a path.
    pub(crate) fn get_url(&self, path: &str) -> crate::Result<Url> {
        self.url
            .join(path.strip_prefix('/').unwrap_or(path))
            .map_err(|e| Error::InvalidValue(format!("invalid url: {e}")))
    }
}

/// Parse a USE description line into a (name, description) tuple.
fn parse_use_desc(s: &str) -> crate::Result<(String, String)> {
    let (flag, desc) = s
        .split_once(" - ")
        .ok_or_else(|| Error::InvalidValue(s.to_string()))?;
    let name = parse::use_flag(flag).map(|s| s.to_string())?;
    Ok((name, desc.to_string()))
}

fn is_eclass(e: &Utf8DirEntry) -> bool {
    is_file_utf8(e) && !is_hidden_utf8(e) && has_ext_utf8(e, "eclass")
}

#[derive(Debug, Default)]
pub struct Metadata {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) eapi: &'static Eapi,
    pub config: Config,
    path: Utf8PathBuf,
    arches: OnceLock<IndexSet<Arch>>,
    arches_desc: OnceLock<IndexMap<ArchStatus, IndexSet<Arch>>>,
    cache: OnceLock<MetadataCache>,
    caches: OnceLock<IndexMap<CacheFormat, MetadataCache>>,
    categories: OnceLock<IndexSet<String>>,
    eclasses: OnceLock<IndexSet<Eclass>>,
    licenses: OnceLock<IndexSet<String>>,
    license_groups: OnceLock<IndexMap<String, IndexSet<String>>>,
    mirrors: OnceLock<IndexMap<String, IndexSet<Mirror>>>,
    pkg_deprecated: OnceLock<IndexSet<Dep>>,
    pkg_mask: OnceLock<IndexSet<Dep>>,
    pkg_metadata_cache: DashMap<Cpn, Arc<xml::Metadata>>,
    pkg_manifest_cache: DashMap<Cpn, Arc<Manifest>>,
    updates: OnceLock<IndexSet<PkgUpdate>>,
    use_global: OnceLock<IndexMap<String, String>>,
    use_expand: OnceLock<IndexMap<String, IndexMap<String, String>>>,
    use_local: OnceLock<OrderedMap<String, OrderedMap<String, String>>>,
}

impl Metadata {
    pub(super) fn try_new(id: &str, path: &Utf8Path) -> crate::Result<Self> {
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
                None => Err(invalid_repo("profiles/repo_name: repo name unset".to_string())),
            },
            Err(e) => {
                let msg = format!("profiles/repo_name: {e}");
                // assume path is misconfigured repo if profiles dir exists
                if path.join("profiles").is_dir() {
                    Err(invalid_repo(msg))
                } else {
                    Err(not_a_repo(msg))
                }
            }
        }?;

        // verify repo EAPI
        let eapi = path
            .join("profiles/eapi")
            .as_path()
            .try_into()
            .map_err(|e| invalid_repo(format!("profiles/eapi: {e}")))?;

        let config = Config::try_new(path)
            .map_err(|e| invalid_repo(format!("metadata/layout.conf: {e}")))?;

        Ok(Self {
            id: id.to_string(),
            name,
            eapi,
            config,
            path: Utf8PathBuf::from(path),
            ..Default::default()
        })
    }

    /// Return a repo's known architectures from `profiles/arch.list`.
    pub fn arches(&self) -> &IndexSet<Arch> {
        self.arches.get_or_init(|| {
            self.read_path("profiles/arch.list")
                .filter_lines()
                .map(|(_, s)| s.into())
                .collect()
        })
    }

    /// Architecture stability status from `profiles/arches.desc`.
    /// See GLEP 72 (https://www.gentoo.org/glep/glep-0072.html).
    pub fn arches_desc(&self) -> &IndexMap<ArchStatus, IndexSet<Arch>> {
        self.arches_desc.get_or_init(|| {
            let mut vals = IndexMap::<_, IndexSet<_>>::new();
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

                        if let Ok(status) = status.parse() {
                            vals.entry(status).or_default().insert(arch.into());
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

    /// Return all metadata caches for a repo.
    pub fn caches(&self) -> &IndexMap<CacheFormat, MetadataCache> {
        self.caches.get_or_init(|| {
            self.config
                .cache_formats
                .iter()
                .copied()
                .map(|format| (format, format.from_repo(&self.path)))
                .collect()
        })
    }

    /// Return the main metadata cache for a repo.
    pub fn cache(&self) -> &MetadataCache {
        self.cache.get_or_init(|| {
            self.caches()
                .values()
                .next()
                .cloned()
                // TODO: use location outside repo for default cache?
                .unwrap_or_else(|| CacheFormat::default().from_repo(&self.path))
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

    /// Return the ordered set of eclasses.
    pub fn eclasses(&self) -> &IndexSet<Eclass> {
        self.eclasses
            .get_or_init(|| match self.path.join("eclass").read_dir_utf8() {
                Ok(entries) => {
                    // TODO: add support for native file reading parallelism
                    let mut vals: IndexSet<_> = entries
                        .filter_map(Result::ok)
                        .filter(is_eclass)
                        .filter_map(|entry| {
                            match Eclass::try_new(entry.path(), self.cache()) {
                                Ok(eclass) => Some(eclass),
                                Err(e) => {
                                    error!("{}: {e}", self.id);
                                    None
                                }
                            }
                        })
                        .collect();
                    vals.sort_unstable();
                    vals
                }
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}: reading eclasses dir failed: {e}", self.id);
                    }
                    Default::default()
                }
            })
    }

    /// Return the ordered set of licenses.
    pub fn licenses(&self) -> &IndexSet<String> {
        self.licenses
            .get_or_init(|| match self.path.join("licenses").read_dir_utf8() {
                Ok(entries) => {
                    let mut vals: IndexSet<_> = entries
                        .filter_map(Result::ok)
                        .filter_map(|e| match parse::license_name(e.file_name()) {
                            Ok(s) => Some(s.to_string()),
                            Err(e) => {
                                error!("{}: {e}", self.id);
                                None
                            }
                        })
                        .collect();
                    vals.sort_unstable();
                    vals
                }
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}: reading licenses dir failed: {e}", self.id);
                    }
                    Default::default()
                }
            })
    }

    /// Return the mapping of license groups.
    pub fn license_groups(&self) -> &IndexMap<String, IndexSet<String>> {
        self.license_groups.get_or_init(|| {
            let mut alias_map = IndexMap::<_, IndexSet<_>>::new();
            let mut group_map = self.read_path("profiles/license_groups")
                .filter_lines()
                .filter_map(|(i, s)| {
                    let mut vals = s.split_whitespace();
                    vals.next().map(|name| {
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
                                            .or_default()
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
                        (name.to_string(), licenses)
                    })
                })
                .collect::<IndexMap<_, IndexSet<_>>>();

            // resolve aliases using DFS
            for (name, aliases) in &alias_map {
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
                                    "{}::profiles/license_groups: {name}: cyclic alias: {x}",
                                    self.id,
                                );
                            }
                        }
                    }

                    // resolve alias values
                    if let Some(values) = group_map.get(&s).cloned() {
                        group_map.entry(name.clone())
                            .or_default()
                            .extend(values);
                    } else {
                        warn!(
                            "{}::profiles/license_groups: {name}: unknown alias: {s}",
                            self.id,
                        );
                    }
                }
            }

            group_map
        })
    }

    /// Return a repo's globally defined mirrors.
    pub fn mirrors(&self) -> &IndexMap<String, IndexSet<Mirror>> {
        // parse a line from profiles/thirdpartymirrors into a set of mirrors
        let parse_line = |lineno: usize, line: &str| -> Option<(String, IndexSet<Mirror>)> {
            // log warning for invalid mirror errors
            let warn = |err: String| {
                warn!("{}::profiles/thirdpartymirrors, line {lineno}: {err}", self.id);
            };

            let mut vals = line.split_whitespace().peekable();
            let name = vals.next().expect("empty mirrors line");

            if vals.peek().is_none() {
                warn(format!("invalid mirror: {name}: no urls"));
                return None;
            }

            let mut mirrors = IndexSet::new();
            for url in vals {
                // force URL to end with / so Url::join() works as expected
                let url = match format!("{}/", url.trim_end_matches('/')).parse() {
                    Ok(url) => url,
                    Err(e) => {
                        warn(format!("invalid mirror url: {url}: {e}"));
                        continue;
                    }
                };
                let mirror = Mirror { name: name.to_string(), url };
                mirrors.insert(mirror);
            }

            if mirrors.is_empty() {
                None
            } else {
                Some((name.to_string(), mirrors))
            }
        };

        self.mirrors.get_or_init(|| {
            self.read_path("profiles/thirdpartymirrors")
                .filter_lines()
                .filter_map(|(i, s)| parse_line(i, s))
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

    /// Return the package metadata result for a [`Cpn`].
    pub fn pkg_metadata(&self, cpn: &Cpn) -> crate::Result<xml::Metadata> {
        let path = build_path!(&self.path, cpn.category(), cpn.package(), "metadata.xml");
        xml::Metadata::from_path(&path).map_err(|e| e.into_unversioned_pkg_err(cpn, &self.id))
    }

    /// Return the cached package manifest for a [`Cpn`].
    pub(crate) fn pkg_metadata_cached(&self, cpn: &Cpn) -> Arc<xml::Metadata> {
        if let Some(value) = self.pkg_metadata_cache.get(cpn) {
            value.clone()
        } else {
            let value = Arc::new(self.pkg_metadata(cpn).unwrap_or_default());
            self.pkg_metadata_cache.insert(cpn.clone(), value.clone());
            value
        }
    }

    /// Return the package manifest result for a [`Cpn`].
    pub fn pkg_manifest(&self, cpn: &Cpn) -> crate::Result<Manifest> {
        let path = build_path!(&self.path, cpn.category(), cpn.package(), "Manifest");
        Manifest::from_path(&path).map_err(|e| e.into_unversioned_pkg_err(cpn, &self.id))
    }

    /// Return the cached package manifest for a [`Cpn`].
    pub(crate) fn pkg_manifest_cached(&self, cpn: &Cpn) -> Arc<Manifest> {
        if let Some(value) = self.pkg_manifest_cache.get(cpn) {
            value.clone()
        } else {
            let value = Arc::new(self.pkg_manifest(cpn).unwrap_or_default());
            self.pkg_manifest_cache.insert(cpn.clone(), value.clone());
            value
        }
    }

    /// Return the ordered set of package updates.
    pub fn updates(&self) -> &IndexSet<PkgUpdate> {
        self.updates.get_or_init(|| {
            sorted_dir_list(self.path.join("profiles/updates"))
                .into_iter()
                .filter_entry(|e| is_file(e) && !is_hidden(e))
                .filter_map(Result::ok)
                .filter_map(|e| fs::read_to_string(e.path()).ok().map(|s| (e, s)))
                .flat_map(|(e, s)| {
                    let file = e.file_name().to_str().unwrap_or_default();
                    // TODO: Note that comments and empty lines are filtered even though
                    // the specification doesn't allow them.
                    s.filter_lines()
                        .filter_map(|(i, line)| {
                            line.parse()
                                .map_err(|err| {
                                    warn!(
                                        "{}::profiles/updates/{file}, line {i}: {err}",
                                        self.id
                                    )
                                })
                                .ok()
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        })
    }

    /// Return the ordered map of global USE flags.
    pub fn use_global(&self) -> &IndexMap<String, String> {
        self.use_global.get_or_init(|| {
            self.read_path("profiles/use.desc")
                .filter_lines()
                .filter_map(|(i, s)| {
                    parse_use_desc(s)
                        .map_err(|e| {
                            warn!(
                                "{}::profiles/use.desc, line {i}: invalid format: {e}",
                                self.id
                            );
                        })
                        .ok()
                })
                .collect()
        })
    }

    /// Return the ordered map of USE_EXPAND flags.
    pub fn use_expand(&self) -> &IndexMap<String, IndexMap<String, String>> {
        self.use_expand.get_or_init(|| {
            sorted_dir_list(self.path.join("profiles/desc"))
                .into_iter()
                .filter_entry(|e| is_file(e) && !is_hidden(e))
                .filter_map(Result::ok)
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
                            parse_use_desc(line)
                                .map_err(|err| {
                                    warn!("{}::profiles/desc/{file}, line {i}: {err}", self.id)
                                })
                                .ok()
                        })
                        .collect();
                    (name.to_string(), vals)
                })
                .collect()
        })
    }

    /// Return the ordered map of local USE flags.
    pub fn use_local(&self) -> &OrderedMap<String, OrderedMap<String, String>> {
        // parse a use.local.desc line
        let parse = |s: &str| -> crate::Result<(String, (String, String))> {
            let (cpn, use_desc) = s
                .split_once(':')
                .ok_or_else(|| Error::InvalidValue(s.to_string()))?;
            let _ = Cpn::try_new(cpn)?;
            Ok((cpn.to_string(), parse_use_desc(use_desc)?))
        };

        self.use_local.get_or_init(|| {
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

    /// Update the local USE flag description cache.
    pub fn use_local_update(&self, repo: &super::EbuildRepo) -> crate::Result<()> {
        // TODO: use native parallel Cpn iterator
        let data = repo
            .categories()
            .into_par_iter()
            .flat_map(|cat| {
                repo.packages(&cat)
                    .into_iter()
                    .map(|pn| Cpn {
                        category: cat.to_string(),
                        package: pn,
                    })
                    .collect::<Vec<_>>()
            })
            .filter_map(|cpn| self.pkg_metadata(&cpn).ok().map(|m| (m, cpn)))
            .collect::<Vec<_>>();

        let mut data = data
            .par_iter()
            .flat_map_iter(|(meta, cpn)| {
                meta.local_use()
                    .iter()
                    .map(|(name, desc)| (cpn, name, desc))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        if !data.is_empty() {
            data.par_sort();
            let data = data
                .iter()
                .map(|(cpn, name, desc)| format!("{cpn}:{name} - {desc}\n"))
                .join("");
            atomic_write_file(self.path.join("profiles/use.local.desc"), data)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use crate::eapi::EAPI_LATEST_OFFICIAL;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::test::*;

    use super::*;

    #[test]
    fn config() {
        // empty config
        let repo = EbuildRepoBuilder::new().build().unwrap();
        fs::write(repo.path().join("metadata/layout.conf"), "").unwrap();
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        assert!(metadata.config.is_empty());

        // empty repo name
        let repo = EbuildRepoBuilder::new().build().unwrap();
        fs::write(repo.path().join("profiles/repo_name"), "").unwrap();
        let r = Metadata::try_new("test", repo.path());
        assert_err_re!(r, "^invalid repo: test: profiles/repo_name: repo name unset$");

        // invalid config
        let repo = EbuildRepoBuilder::new().build().unwrap();
        fs::write(repo.path().join("metadata/layout.conf"), "data").unwrap();
        let r = Metadata::try_new("test", repo.path());
        assert_err_re!(r, "^invalid repo: test: metadata/layout.conf: failed parsing INI: ");
    }

    #[test]
    fn arches() {
        let repo = EbuildRepoBuilder::new().build().unwrap();

        // nonexistent file
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        assert!(metadata.arches().is_empty());

        // empty file
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "").unwrap();
        assert!(metadata.arches().is_empty());

        // multiple
        let data = indoc::indoc! {r#"
            amd64
            arm64
            amd64-linux
        "#};
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), data).unwrap();
        assert_ordered_eq!(metadata.arches(), ["amd64", "arm64", "amd64-linux"]);
    }

    #[traced_test]
    #[test]
    fn arches_desc() {
        let repo = EbuildRepoBuilder::new().build().unwrap();

        // nonexistent file
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        assert!(metadata.arches_desc().is_empty());

        // empty file
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "").unwrap();
        assert!(metadata.arches_desc().is_empty());

        // invalid line format
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64\narm64").unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "amd64 stable\narm64").unwrap();
        assert!(!metadata.arches_desc().is_empty());
        assert_logs_re!(".+, line 2: invalid line format: .+$");

        // unknown arch
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64").unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "arm64 stable").unwrap();
        assert!(metadata.arches_desc().is_empty());
        assert_logs_re!(".+, line 1: unknown arch: arm64$");

        // unknown status
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64").unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "amd64 test").unwrap();
        assert!(metadata.arches_desc().is_empty());
        assert_logs_re!(".+, line 1: unknown status: test$");

        // multiple with ignored 3rd column
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64\narm64\nppc\nppc64")
            .unwrap();
        fs::write(
            metadata.path.join("profiles/arches.desc"),
            "amd64 stable\narm64 testing\nppc testing\nppc64 transitional 3rd-col",
        )
        .unwrap();
        assert_ordered_eq!(&metadata.arches_desc()[&ArchStatus::Stable], ["amd64"]);
        assert_ordered_eq!(&metadata.arches_desc()[&ArchStatus::Testing], ["arm64", "ppc"]);
        assert_ordered_eq!(&metadata.arches_desc()[&ArchStatus::Transitional], ["ppc64"]);
    }

    #[traced_test]
    #[test]
    fn categories() {
        let repo = EbuildRepoBuilder::new().build().unwrap();

        // nonexistent file
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        assert!(metadata.categories().is_empty());

        // empty file
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/categories"), "").unwrap();
        assert!(metadata.categories().is_empty());

        // multiple with invalid entry
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/categories"), "cat\nc@t").unwrap();
        assert_ordered_eq!(metadata.categories(), ["cat"]);
        assert_logs_re!(".+, line 2: .* invalid category name: c@t$");

        // multiple
        let data = indoc::indoc! {r#"
            cat1
            cat2
            cat-3
        "#};
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/categories"), data).unwrap();
        assert_ordered_eq!(metadata.categories(), ["cat1", "cat2", "cat-3"]);
    }

    #[test]
    fn eclasses() {
        let data = test_data();
        let repo = data.ebuild_repo("secondary").unwrap();
        // uninherited eclasses
        assert_ordered_eq!(repo.metadata().eclasses().iter().map(|e| e.name()), ["b", "c"]);
        // inherited eclasses
        assert_ordered_eq!(repo.eclasses().iter().map(|e| e.name()), ["a", "b", "c"]);
    }

    #[test]
    fn licenses() {
        let repo = EbuildRepoBuilder::new().build().unwrap();

        // nonexistent dir
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        assert!(metadata.licenses().is_empty());

        // empty dir
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::create_dir(metadata.path.join("licenses")).unwrap();
        assert!(metadata.licenses().is_empty());

        // multiple
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("licenses/L1"), "").unwrap();
        fs::write(metadata.path.join("licenses/L2"), "").unwrap();
        assert_ordered_eq!(metadata.licenses(), ["L1", "L2"]);
    }

    #[traced_test]
    #[test]
    fn license_groups() {
        let repo = EbuildRepoBuilder::new().build().unwrap();

        // nonexistent dir
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        assert!(metadata.license_groups().is_empty());

        // empty file
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
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
            group1 a b

            # comment 2
            group2 a	z
        "#};
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/license_groups"), data).unwrap();
        assert_ordered_eq!(metadata.license_groups().get("group1").unwrap(), ["a", "b"]);
        assert_ordered_eq!(metadata.license_groups().get("group2").unwrap(), ["a"]);
        assert_logs_re!(".+, line 5: unknown license: z$");

        // multiple with unknown and invalid aliases
        let data = indoc::indoc! {r#"
            # comment 1
            group1 b @

            # comment 2
            group2 a c @group1 @group3
        "#};
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/license_groups"), data).unwrap();
        assert_ordered_eq!(metadata.license_groups().get("group1").unwrap(), ["b"]);
        assert_ordered_eq!(metadata.license_groups().get("group2").unwrap(), ["a", "c", "b"]);
        assert_logs_re!(".+, line 2: invalid alias: @");
        assert_logs_re!(".+ group2: unknown alias: group3");

        // multiple with cyclic aliases
        let data = indoc::indoc! {r#"
            group1 a @group2
            group2 b @group1
            group3 c @group2
            group4 c @group4
        "#};
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/license_groups"), data).unwrap();
        assert_ordered_eq!(metadata.license_groups().get("group1").unwrap(), ["a", "b"]);
        assert_ordered_eq!(metadata.license_groups().get("group2").unwrap(), ["b", "a"]);
        assert_ordered_eq!(metadata.license_groups().get("group3").unwrap(), ["c", "b", "a"]);
        assert_ordered_eq!(metadata.license_groups().get("group4").unwrap(), ["c"]);
        assert_logs_re!(".+ group1: cyclic alias: group2");
        assert_logs_re!(".+ group2: cyclic alias: group1");
        assert_logs_re!(".+ group3: cyclic alias: group2");
        assert_logs_re!(".+ group4: cyclic alias: group4");
    }

    #[traced_test]
    #[test]
    fn mirrors() {
        let repo = EbuildRepoBuilder::new().build().unwrap();

        // nonexistent file
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        assert!(metadata.mirrors().is_empty());

        // empty file
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/thirdpartymirrors"), "").unwrap();
        assert!(metadata.mirrors().is_empty());

        // multiple with mixed whitespace
        let data = indoc::indoc! {r#"
            # mirror with no urls
            invalid1

            # mirror with invalid url
            invalid2 invalid-url

            # comment 1
            mirror1 https://a/mirror/ https://another/mirror

            # comment 2
            mirror2	http://yet/another/mirror/

            # mirror with both valid and invalid url
            mirror3 https://b/mirror invalid-url
        "#};
        let metadata = Metadata::try_new("test", repo.path()).unwrap();
        fs::write(metadata.path.join("profiles/thirdpartymirrors"), data).unwrap();
        let mirrors = metadata.mirrors();
        assert_eq!(mirrors.len(), 3);
        for (name, expected) in [
            ("mirror1", vec!["https://a/mirror/", "https://another/mirror/"]),
            ("mirror2", vec!["http://yet/another/mirror/"]),
            ("mirror3", vec!["https://b/mirror/"]),
        ] {
            assert_ordered_eq!(
                mirrors.get(name).unwrap().iter().map(|x| x.url.as_str()),
                expected
            );
        }

        // verify log messages for invalid entries
        let path = "test::profiles/thirdpartymirrors";
        assert_logs_re!("^.+: {path}, line 2: invalid mirror: invalid1: no urls$");
        assert_logs_re!("^.+: {path}, line 5: invalid mirror url: invalid-url: .+$");
        assert_logs_re!("^.+: {path}, line 14: invalid mirror url: invalid-url: .+$");
    }

    #[traced_test]
    #[test]
    fn pkg_deprecated() {
        let temp = EbuildRepoBuilder::new().build().unwrap();

        // nonexistent file
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        assert!(metadata.pkg_deprecated().is_empty());

        // empty file
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.deprecated"), "").unwrap();
        assert!(metadata.pkg_deprecated().is_empty());

        // multiple with invalid dep for repo EAPI
        let data = indoc::indoc! {r#"
            # comment 1
            cat/pkg-a

            # comment 2
            another/pkg

            # invalid
            cat/pkg-1
        "#};
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.deprecated"), data).unwrap();
        assert_ordered_eq!(
            metadata.pkg_deprecated().clone(),
            [Dep::try_new("cat/pkg-a").unwrap(), Dep::try_new("another/pkg").unwrap()],
        );
        assert_logs_re!(".+, line 8: .* invalid dep: cat/pkg-1$");

        // newer repo EAPI allows using newer dep format features
        let temp = EbuildRepoBuilder::new()
            .eapi(&EAPI_LATEST_OFFICIAL)
            .build()
            .unwrap();
        // multiple with invalid dep for repo EAPI
        let data = indoc::indoc! {r#"
            cat/slotted:0
            cat/subslot:0/1
        "#};
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.deprecated"), data).unwrap();
        assert_ordered_eq!(
            metadata.pkg_deprecated().clone(),
            [Dep::try_new("cat/slotted:0").unwrap(), Dep::try_new("cat/subslot:0/1").unwrap()],
        );
    }

    #[traced_test]
    #[test]
    fn pkg_mask() {
        let temp = EbuildRepoBuilder::new().build().unwrap();

        // nonexistent file
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        assert!(metadata.pkg_mask().is_empty());

        // empty file
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.mask"), "").unwrap();
        assert!(metadata.pkg_mask().is_empty());

        // multiple with invalid dep for repo EAPI
        let data = indoc::indoc! {r#"
            # comment 1
            cat/pkg-a

            # comment 2
            another/pkg

            # invalid
            cat/pkg-1
        "#};
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.mask"), data).unwrap();
        assert_ordered_eq!(
            metadata.pkg_mask().clone(),
            [Dep::try_new("cat/pkg-a").unwrap(), Dep::try_new("another/pkg").unwrap()],
        );
        assert_logs_re!(".+, line 8: .* invalid dep: cat/pkg-1$");

        // newer repo EAPI allows using newer dep format features
        let temp = EbuildRepoBuilder::new()
            .eapi(&EAPI_LATEST_OFFICIAL)
            .build()
            .unwrap();
        // multiple with invalid dep for repo EAPI
        let data = indoc::indoc! {r#"
            cat/slotted:0
            cat/subslot:0/1
        "#};
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        fs::write(metadata.path.join("profiles/package.mask"), data).unwrap();
        assert_ordered_eq!(
            metadata.pkg_mask().clone(),
            [Dep::try_new("cat/slotted:0").unwrap(), Dep::try_new("cat/subslot:0/1").unwrap()],
        );
    }

    #[traced_test]
    #[test]
    fn updates() {
        let temp = EbuildRepoBuilder::new().build().unwrap();

        // nonexistent
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        assert!(metadata.updates().is_empty());

        // empty
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
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
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        fs::write(metadata.path.join("profiles/updates/1Q-9999"), data).unwrap();
        let updates = metadata.updates();
        assert_eq!(updates.len(), 2);
        assert_logs_re!(".+ line 5: .+?: invalid cpn: cat/pkg3-1$");
        assert_logs_re!(".+ line 11: .+?: invalid slot: @$");
    }

    #[traced_test]
    #[test]
    fn use_global() {
        let temp = EbuildRepoBuilder::new().build().unwrap();

        // nonexistent
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        assert!(metadata.use_global().is_empty());

        // empty
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        fs::write(metadata.path.join("profiles/use.desc"), "").unwrap();
        assert!(metadata.use_global().is_empty());

        // multiple with invalid
        let data = indoc::indoc! {r#"
            # normal
            a - a flag description

            # invalid format
            b: b flag description

            # invalid USE flag
            @c - c flag description
        "#};
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        fs::write(metadata.path.join("profiles/use.desc"), data).unwrap();
        assert_eq!(metadata.use_global().get("a").unwrap(), "a flag description");
        assert_logs_re!(".+ line 5: invalid format: b: b flag description$");
        assert_logs_re!(".+ line 8: .+?: invalid USE flag: @c$");
    }

    #[traced_test]
    #[test]
    fn use_local() {
        let temp = EbuildRepoBuilder::new().build().unwrap();

        // nonexistent
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        assert!(metadata.use_local().is_empty());

        // empty
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        fs::write(metadata.path.join("profiles/use.local.desc"), "").unwrap();
        assert!(metadata.use_local().is_empty());

        // multiple with invalid
        let data = indoc::indoc! {r#"
            # normal
            cat/pkg:a - a flag description

            # invalid format
            b - b flag description

            # invalid USE flag
            cat/pkg:@c - c flag description
        "#};
        let metadata = Metadata::try_new("test", temp.path()).unwrap();
        fs::write(metadata.path.join("profiles/use.local.desc"), data).unwrap();
        assert_eq!(
            metadata
                .use_local()
                .get("cat/pkg")
                .unwrap()
                .get("a")
                .unwrap(),
            "a flag description"
        );
        assert_logs_re!(".+ line 5: invalid format: b - b flag description$");
        assert_logs_re!(".+ line 8: .+?: invalid USE flag: @c$");
    }
}
