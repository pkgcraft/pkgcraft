use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};

use camino::Utf8Path;
use itertools::Itertools;
use roxmltree::{Document, Node};

use crate::macros::cmp_not_equal;
use crate::repo::ebuild::CacheData;
use crate::Error;

#[derive(Debug)]
pub struct Maintainer {
    email: Option<String>,
    name: Option<String>,
    description: Option<String>,
    maint_type: Option<String>,
    proxied: Option<String>,
}

impl Maintainer {
    fn new(
        email: Option<&str>,
        name: Option<&str>,
        description: Option<&str>,
        maint_type: Option<&str>,
        proxied: Option<&str>,
    ) -> crate::Result<Self> {
        if email.is_none() && name.is_none() {
            return Err(Error::InvalidValue("either email or name must exist".to_string()));
        }

        Ok(Self {
            email: email.map(String::from),
            name: name.map(String::from),
            description: description.map(String::from),
            maint_type: maint_type.map(String::from),
            proxied: proxied.map(String::from),
        })
    }

    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn maint_type(&self) -> Option<&str> {
        self.maint_type.as_deref()
    }

    pub fn proxied(&self) -> Option<&str> {
        self.proxied.as_deref()
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Upstream {
    site: String,
    name: String,
}

impl Upstream {
    fn new(site: &str, name: &str) -> Self {
        Self {
            site: site.to_string(),
            name: name.to_string(),
        }
    }

    pub fn site(&self) -> &str {
        &self.site
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Default)]
pub struct XmlMetadata {
    maintainers: Vec<Maintainer>,
    upstreams: Vec<Upstream>,
    local_use: HashMap<String, String>,
    long_desc: Option<String>,
}

impl CacheData for XmlMetadata {
    fn new(path: &Utf8Path) -> Self {
        match fs::read_to_string(path.join("metadata.xml")) {
            Err(_) => Self::default(),
            Ok(s) => Self::parse_xml(&s),
        }
    }
}

impl XmlMetadata {
    fn parse_maintainer(node: Node, data: &mut Self) {
        let (mut email, mut name, mut description) = (None, None, None);
        for n in node.children() {
            match n.tag_name().name() {
                "email" => email = n.text(),
                "name" => name = n.text(),
                "description" => description = n.text(),
                _ => (),
            }
        }
        let maint_type = node.attribute("type");
        let proxied = node.attribute("proxied");
        if let Ok(m) = Maintainer::new(email, name, description, maint_type, proxied) {
            data.maintainers.push(m);
        }
    }

    fn parse_upstreams(node: Node, data: &mut Self) {
        let nodes = node
            .children()
            .filter(|n| n.tag_name().name() == "remote-id");
        for n in nodes {
            if let (Some(site), Some(name)) = (n.attribute("type"), n.text()) {
                data.upstreams.push(Upstream::new(site, name));
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
        data.long_desc = node.text().map(|s| {
            let (text, _opts) = textwrap::unfill(textwrap::dedent(s).trim());
            text
        });
    }

    fn parse_xml(xml: &str) -> Self {
        let mut data = Self::default();
        if let Ok(doc) = Document::parse(xml) {
            for node in doc.descendants() {
                let lang = node.attribute("lang").unwrap_or("en");
                let en = lang == "en";
                match node.tag_name().name() {
                    "maintainer" => Self::parse_maintainer(node, &mut data),
                    "upstream" => Self::parse_upstreams(node, &mut data),
                    "use" if en => Self::parse_use(node, &mut data),
                    "longdescription" if en => Self::parse_long_desc(node, &mut data),
                    _ => (),
                }
            }
        }
        data
    }

    pub(crate) fn maintainers(&self) -> &[Maintainer] {
        &self.maintainers
    }

    pub(crate) fn upstreams(&self) -> &[Upstream] {
        &self.upstreams
    }

    pub(crate) fn local_use(&self) -> &HashMap<String, String> {
        &self.local_use
    }

    pub(crate) fn long_desc(&self) -> Option<&str> {
        self.long_desc.as_deref()
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Distfile {
    name: String,
    size: u64,
    checksums: Vec<(String, String)>,
}

impl Distfile {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn checksums(&self) -> &[(String, String)] {
        &self.checksums
    }
}

#[derive(Debug, Default)]
pub struct Manifest {
    dist: Vec<Distfile>,
}

impl CacheData for Manifest {
    fn new(path: &Utf8Path) -> Self {
        match fs::read_to_string(path.join("Manifest")) {
            Err(_) => Self::default(),
            Ok(s) => Self::parse_manifest(&s),
        }
    }
}

impl Manifest {
    // TODO: handle error checking
    fn parse_manifest(data: &str) -> Self {
        let mut dist = vec![];
        for line in data.lines() {
            let mut fields = line.split_whitespace();
            // TODO: support other field types
            if let Some("DIST") = fields.next() {
                let filename = fields.next().unwrap();
                let size = fields.next().unwrap();
                let checksums = fields
                    .tuples()
                    .map(|(s, val)| (s.to_ascii_lowercase(), val.to_string()))
                    .collect::<Vec<(String, String)>>();
                dist.push(Distfile {
                    name: filename.to_string(),
                    size: size.parse().unwrap(),
                    checksums,
                })
            }
        }
        Manifest { dist }
    }

    pub(crate) fn distfiles(&self) -> &[Distfile] {
        &self.dist
    }
}
