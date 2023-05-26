use std::cmp::Ordering;
use std::collections::{hash_map, HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use camino::Utf8Path;
use itertools::Itertools;
use roxmltree::{Document, Node};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator};

use crate::macros::{build_from_paths, cmp_not_equal};
use crate::repo::ebuild::CacheData;
use crate::types::OrderedSet;
use crate::utils::digest;
use crate::Error;

#[derive(AsRefStr, Display, EnumString, Debug, Default, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum MaintainerType {
    #[default]
    Person,
    Project,
}

#[derive(AsRefStr, Display, EnumString, Debug, Default, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum Proxied {
    Proxy,
    Yes,
    #[default]
    No,
}

#[derive(Debug, Default, Clone)]
pub struct Maintainer {
    email: String,
    name: Option<String>,
    description: Option<String>,
    maint_type: MaintainerType,
    proxied: Proxied,
}

impl Maintainer {
    pub fn email(&self) -> &str {
        &self.email
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn maint_type(&self) -> MaintainerType {
        self.maint_type
    }

    pub fn proxied(&self) -> Proxied {
        self.proxied
    }
}

impl PartialEq for Maintainer {
    fn eq(&self, other: &Self) -> bool {
        self.email == other.email && self.name == other.name
    }
}

impl Eq for Maintainer {}

impl Ord for Maintainer {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp_not_equal!(&self.email, &other.email);
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for Maintainer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hash for Maintainer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.email.hash(state);
        self.name.hash(state);
    }
}

/// Convert &str to Option<String> with whitespace-only strings returning None.
fn string_or_none(s: &str) -> Option<String> {
    match s.trim() {
        "" => None,
        s => Some(s.to_string()),
    }
}

/// Convert Option<&str> to String with None mapping to the empty string.
fn string_or_empty(s: Option<&str>) -> String {
    s.map(|s| s.trim()).unwrap_or_default().to_string()
}

impl TryFrom<Node<'_, '_>> for Maintainer {
    type Error = Error;

    fn try_from(node: Node<'_, '_>) -> Result<Self, Self::Error> {
        let mut maintainer = Maintainer::default();

        for n in node.children() {
            match n.tag_name().name() {
                "email" => maintainer.email = string_or_empty(n.text()),
                "name" => maintainer.name = n.text().and_then(string_or_none),
                "description" => maintainer.description = n.text().and_then(string_or_none),
                _ => (),
            }
        }

        let maint_type = node.attribute("type").unwrap_or_default();
        maintainer.maint_type = MaintainerType::from_str(maint_type).unwrap_or_default();
        let proxied = node.attribute("proxied").unwrap_or_default();
        maintainer.proxied = Proxied::from_str(proxied).unwrap_or_default();

        if maintainer.email.is_empty() {
            return Err(Error::InvalidValue("maintainer missing required email".to_string()));
        }

        Ok(maintainer)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RemoteId {
    site: String,
    name: String,
}

impl RemoteId {
    pub fn site(&self) -> &str {
        &self.site
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Display, EnumString, Debug, Default, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum MaintainerStatus {
    Active,
    Inactive,
    #[default]
    Unknown,
}

#[derive(Debug, Default, Clone)]
pub struct UpstreamMaintainer {
    name: String,
    email: Option<String>,
    status: MaintainerStatus,
}

impl UpstreamMaintainer {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    pub fn status(&self) -> MaintainerStatus {
        self.status
    }
}

#[derive(Debug, Default, Clone)]
pub struct Upstream {
    remote_ids: OrderedSet<RemoteId>,
    maintainers: Vec<UpstreamMaintainer>,
    bugs_to: Option<String>,
    changelog: Option<String>,
    doc: Option<String>,
}

impl Upstream {
    pub fn remote_ids(&self) -> &OrderedSet<RemoteId> {
        &self.remote_ids
    }

    pub fn maintainers(&self) -> &[UpstreamMaintainer] {
        &self.maintainers
    }

    pub fn bugs_to(&self) -> Option<&str> {
        self.bugs_to.as_deref()
    }

    pub fn changelog(&self) -> Option<&str> {
        self.changelog.as_deref()
    }

    pub fn doc(&self) -> Option<&str> {
        self.doc.as_deref()
    }
}

impl TryFrom<Node<'_, '_>> for Upstream {
    type Error = Error;

    fn try_from(node: Node<'_, '_>) -> Result<Self, Self::Error> {
        let mut upstream = Upstream::default();

        for u_child in node.children() {
            match u_child.tag_name().name() {
                "maintainer" => {
                    let mut m = UpstreamMaintainer::default();
                    let status = u_child.attribute("status").unwrap_or_default();
                    m.status = MaintainerStatus::from_str(status).unwrap_or_default();
                    for m_child in u_child.children() {
                        match m_child.tag_name().name() {
                            "name" => m.name = string_or_empty(m_child.text()),
                            "email" => m.email = m_child.text().and_then(string_or_none),
                            _ => (),
                        }
                    }
                    upstream.maintainers.push(m);
                }
                "bugs-to" => upstream.bugs_to = u_child.text().and_then(string_or_none),
                "changelog" => upstream.changelog = u_child.text().and_then(string_or_none),
                "doc" => upstream.doc = u_child.text().and_then(string_or_none),
                "remote-id" => {
                    if let (Some(site), Some(name)) = (u_child.attribute("type"), u_child.text()) {
                        let r = RemoteId {
                            site: site.to_string(),
                            name: name.to_string(),
                        };
                        upstream.remote_ids.insert(r);
                    }
                }
                _ => (),
            }
        }

        Ok(upstream)
    }
}

/// Package metadata contained in metadata.xml files as defined by GLEP 68.
#[derive(Debug, Default, Clone)]
pub struct XmlMetadata {
    maintainers: Vec<Maintainer>,
    upstream: Option<Upstream>,
    slots: HashMap<String, String>,
    subslots: Option<String>,
    stabilize_allarches: bool,
    local_use: HashMap<String, String>,
    long_desc: Option<String>,
}

impl CacheData for XmlMetadata {
    const RELPATH: &'static str = "metadata.xml";

    fn parse(data: &str) -> crate::Result<Self> {
        let doc = Document::parse(data).map_err(|e| Error::InvalidValue(e.to_string()))?;
        let mut data = Self::default();

        for node in doc.root_element().children() {
            let lang = node.attribute("lang").unwrap_or("en");
            let en = lang == "en";
            match node.tag_name().name() {
                "maintainer" => data.maintainers.push(node.try_into()?),
                "upstream" => data.upstream = Some(node.try_into()?),
                "slots" => Self::parse_slots(node, &mut data),
                "stabilize-allarches" => data.stabilize_allarches = true,
                "use" if en => Self::parse_use(node, &mut data),
                "longdescription" if en => Self::parse_long_desc(node, &mut data),
                _ => (),
            }
        }

        Ok(data)
    }
}

impl XmlMetadata {
    fn parse_slots(node: Node, data: &mut Self) {
        for n in node.children() {
            match (n.tag_name().name(), n.text().and_then(string_or_none)) {
                ("slot", Some(desc)) => {
                    if let Some(name) = n.attribute("name") {
                        data.slots.insert(name.to_string(), desc);
                    }
                }
                ("subslots", desc @ Some(_)) => data.subslots = desc,
                _ => (),
            }
        }
    }

    fn parse_use(node: Node, data: &mut Self) {
        let nodes = node.children().filter(|n| n.tag_name().name() == "flag");
        for n in nodes {
            if let (Some(name), Some(desc)) = (n.attribute("name"), n.text()) {
                data.local_use.insert(name.to_string(), desc.to_string());
            }
        }
    }

    fn parse_long_desc(node: Node, data: &mut Self) {
        data.long_desc = node.text().map(|s| s.split_whitespace().join(" "));
    }

    /// Return a package's maintainers.
    pub fn maintainers(&self) -> &[Maintainer] {
        &self.maintainers
    }

    /// Return a package's upstream info.
    pub fn upstream(&self) -> Option<&Upstream> {
        self.upstream.as_ref()
    }

    /// Return a package's slot descriptions.
    pub fn slots(&self) -> &HashMap<String, String> {
        &self.slots
    }

    /// Return a package's subslots description.
    pub fn subslots(&self) -> Option<&str> {
        self.subslots.as_deref()
    }

    /// Return a package's architecture-independent status.
    pub fn stabilize_allarches(&self) -> bool {
        self.stabilize_allarches
    }

    /// Return a package's local USE flag mapping.
    pub fn local_use(&self) -> &HashMap<String, String> {
        &self.local_use
    }

    /// Return a package's long description.
    pub fn long_description(&self) -> Option<&str> {
        self.long_desc.as_deref()
    }
}

#[derive(Display, EnumString, Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Copy, Clone)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum HashType {
    Blake2b,
    Blake3,
    Sha512,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Checksum {
    kind: HashType,
    value: String,
}

impl Checksum {
    pub(super) fn new(kind: &str, value: &str) -> crate::Result<Self> {
        let kind = HashType::from_str(kind)
            .map_err(|_| Error::InvalidValue(format!("unknown checksum kind: {kind}")))?;
        Ok(Checksum { kind, value: value.to_string() })
    }

    /// Verify the checksum matches the given data.
    fn verify(&self, data: &[u8]) -> crate::Result<()> {
        let new = match self.kind {
            HashType::Blake2b => digest::<blake2::Blake2b512>(data),
            HashType::Blake3 => digest::<blake3::Hasher>(data),
            HashType::Sha512 => digest::<sha2::Sha512>(data),
        };

        if self.value != new {
            return Err(Error::InvalidValue(format!(
                "{} checksum failed: orig: {}, new: {new}",
                self.kind, self.value
            )));
        }

        Ok(())
    }
}

#[derive(
    Display, EnumString, EnumIter, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ManifestType {
    Aux,
    Dist,
    Ebuild,
    Misc,
}

/// Package manifest contained in Manifest files as defined by GLEP 44.
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Clone)]
pub struct ManifestFile {
    kind: ManifestType,
    name: String,
    size: u64,
    checksums: Vec<Checksum>,
}

impl ManifestFile {
    fn new(kind: ManifestType, name: &str, size: u64, chksums: &[&str]) -> crate::Result<Self> {
        let checksums: crate::Result<Vec<_>> = chksums
            .iter()
            .tuples()
            .map(|(kind, val)| Checksum::new(kind, val))
            .collect();

        Ok(ManifestFile {
            kind,
            name: name.to_string(),
            size,
            checksums: checksums?,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn checksums(&self) -> &[Checksum] {
        &self.checksums
    }

    pub fn verify(
        &self,
        required_hashes: &OrderedSet<HashType>,
        pkgdir: &Utf8Path,
        distdir: &Utf8Path,
    ) -> crate::Result<()> {
        let path = match self.kind {
            ManifestType::Aux => build_from_paths!(pkgdir, "files", &self.name),
            ManifestType::Dist => distdir.join(&self.name),
            _ => pkgdir.join(&self.name),
        };
        let data =
            fs::read(&path).map_err(|e| Error::IO(format!("failed verifying: {path}: {e}")))?;

        self.checksums
            .iter()
            .filter(|c| required_hashes.contains(&c.kind))
            .try_for_each(|c| c.verify(&data))
    }
}

#[derive(Debug, Clone)]
pub struct Manifest {
    files: HashMap<ManifestType, HashSet<ManifestFile>>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            files: ManifestType::iter().map(|t| (t, HashSet::new())).collect(),
        }
    }
}

impl CacheData for Manifest {
    const RELPATH: &'static str = "Manifest";

    fn parse(data: &str) -> crate::Result<Self> {
        let mut manifest = Self::default();

        for (i, line) in data.lines().enumerate() {
            let fields: Vec<_> = line.split_whitespace().collect();

            // verify manifest tokens include at least one checksum
            if fields.len() < 5 || (fields.len() % 2 == 0) {
                return Err(Error::InvalidValue(format!(
                    "line {}, invalid number of manifest tokens",
                    i + 1,
                )));
            }

            let kind = ManifestType::from_str(fields[0])
                .map_err(|e| Error::InvalidValue(e.to_string()))?;
            let name = &fields[1];
            let size = fields[2]
                .parse()
                .map_err(|e| Error::InvalidValue(format!("line {}, invalid size: {e}", i + 1)))?;
            manifest
                .files
                .entry(kind)
                .or_insert_with(HashSet::new)
                .insert(ManifestFile::new(kind, name, size, &fields[3..])?);
        }

        if manifest.is_empty() {
            return Err(Error::InvalidValue("empty Manifest".to_string()));
        }

        Ok(manifest)
    }
}

impl Manifest {
    pub fn distfiles(&self) -> &HashSet<ManifestFile> {
        self.files
            .get(&ManifestType::Dist)
            .expect("invalid ManifestFile::default()")
    }

    pub fn is_empty(&self) -> bool {
        self.files.values().all(|s| s.is_empty())
    }

    pub fn verify(
        &self,
        required_hashes: &OrderedSet<HashType>,
        pkgdir: &Utf8Path,
        distdir: &Utf8Path,
    ) -> crate::Result<()> {
        self.into_iter()
            .try_for_each(|f| f.verify(required_hashes, pkgdir, distdir))
    }
}

impl<'a> IntoIterator for &'a Manifest {
    type Item = &'a ManifestFile;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            iter: self.files.values().flatten(),
        }
    }
}

pub struct Iter<'a> {
    iter: std::iter::Flatten<hash_map::Values<'a, ManifestType, HashSet<ManifestFile>>>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a ManifestFile;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::macros::assert_err_re;

    use super::*;

    #[test]
    fn test_distfile_verification() {
        let mut config = crate::config::Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        let manifest_hashes = t.repo().metadata().config().manifest_hashes();
        let required_hashes = t.repo().metadata().config().manifest_required_hashes();
        let tmpdir = tempdir().unwrap();
        let distdir: &Utf8Path = tmpdir.path().try_into().unwrap();

        // empty
        let r = Manifest::parse("");
        assert_err_re!(r, "empty Manifest");

        // missing distfile
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B a SHA512 b
        "#};
        let manifest = Manifest::parse(data).unwrap();
        let r = manifest.verify(required_hashes, distdir, distdir);
        assert_err_re!(r, "No such file or directory");

        // failing primary checksum
        fs::write(distdir.join("a.tar.gz"), "value").unwrap();
        let r = manifest.verify(required_hashes, distdir, distdir);
        assert_err_re!(r, "BLAKE2B checksum failed");

        // secondary checksum failure is ignored since it's not in manifest-required-hashes for the repo
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B 631ad87bd3f552d3454be98da63b68d13e55fad21cad040183006b52fce5ceeaf2f0178b20b3966447916a330930a8754c2ef1eed552e426a7e158f27a4668c5 SHA512 b
        "#};
        let manifest = Manifest::parse(data).unwrap();
        assert!(manifest.verify(required_hashes, distdir, distdir).is_ok());

        // secondary checksum failure due to including it in the required hashes param
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B 631ad87bd3f552d3454be98da63b68d13e55fad21cad040183006b52fce5ceeaf2f0178b20b3966447916a330930a8754c2ef1eed552e426a7e158f27a4668c5 SHA512 b
        "#};
        let manifest = Manifest::parse(data).unwrap();
        let r = manifest.verify(manifest_hashes, distdir, distdir);
        assert_err_re!(r, "SHA512 checksum failed");

        // verified
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B 631ad87bd3f552d3454be98da63b68d13e55fad21cad040183006b52fce5ceeaf2f0178b20b3966447916a330930a8754c2ef1eed552e426a7e158f27a4668c5 SHA512 ec2c83edecb60304d154ebdb85bdfaf61a92bd142e71c4f7b25a15b9cb5f3c0ae301cfb3569cf240e4470031385348bc296d8d99d09e06b26f09591a97527296
        "#};
        let manifest = Manifest::parse(data).unwrap();
        assert!(manifest.verify(required_hashes, distdir, distdir).is_ok());
    }
}
