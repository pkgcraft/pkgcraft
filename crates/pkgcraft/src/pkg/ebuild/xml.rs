use std::cmp::Ordering;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use itertools::Itertools;
use roxmltree::Node;
use strum::{AsRefStr, Display, EnumString};

use crate::macros::cmp_not_equal;
use crate::repo::ebuild::ArcCacheData;
use crate::types::OrderedSet;
use crate::xml::parse_xml_with_dtd;
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
        maintainer.maint_type = maint_type.parse().unwrap_or_default();
        let proxied = node.attribute("proxied").unwrap_or_default();
        maintainer.proxied = proxied.parse().unwrap_or_default();

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
                    m.status = status.parse().unwrap_or_default();
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
pub(crate) struct Metadata {
    pub(super) maintainers: Vec<Maintainer>,
    pub(super) upstream: Option<Upstream>,
    pub(super) slots: HashMap<String, String>,
    pub(super) subslots: Option<String>,
    pub(super) stabilize_allarches: bool,
    pub(super) local_use: HashMap<String, String>,
    pub(super) long_desc: Option<String>,
}

impl ArcCacheData for Metadata {
    const RELPATH: &'static str = "metadata.xml";

    fn parse(data: &str) -> crate::Result<Self> {
        let doc = parse_xml_with_dtd(data).map_err(|e| Error::InvalidValue(e.to_string()))?;
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

impl Metadata {
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
}
