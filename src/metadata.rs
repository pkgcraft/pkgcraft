use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::{fs, io};

use camino::Utf8Path;
use indexmap::IndexSet;
use itertools::Itertools;
use scallop::{functions, variables};
use strum::{AsRefStr, Display, EnumString};
use tracing::warn;

use crate::atom::Atom;
use crate::depset::{DepSet, Uri};
use crate::eapi::Eapi;
use crate::macros::build_from_paths;
use crate::pkgsh::{source_ebuild, BuildData, BUILD_DATA};
use crate::repo::{ebuild::Repo as EbuildRepo, Repository};
use crate::Error;

pub mod ebuild;

#[derive(AsRefStr, EnumString, Display, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum Key {
    Iuse,
    RequiredUse,
    Depend,
    Rdepend,
    Pdepend,
    Bdepend,
    Idepend,
    Properties,
    Restrict,
    Description,
    Slot,
    DefinedPhases,
    Eapi,
    Homepage,
    Inherit,
    Inherited,
    Keywords,
    License,
    SrcUri,
}

impl Key {
    pub(crate) fn get(&self, eapi: &'static Eapi) -> Option<String> {
        match self {
            Key::DefinedPhases => {
                let mut phase_names: Vec<_> = eapi
                    .phases()
                    .iter()
                    .filter_map(|p| functions::find(p).map(|_| p.short_name()))
                    .collect();
                match phase_names.is_empty() {
                    true => None,
                    false => {
                        phase_names.sort_unstable();
                        Some(phase_names.join(" "))
                    }
                }
            }
            Key::Inherit => BUILD_DATA.with(|d| {
                let inherit = &d.borrow().inherit;
                match inherit.is_empty() {
                    true => None,
                    false => Some(inherit.iter().join(" ")),
                }
            }),
            key => variables::optional(key).map(|s| s.split_whitespace().join(" ")),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Metadata {
    description: String,
    slot: String,
    deps: HashMap<Key, DepSet<Atom>>,
    license: Option<DepSet<String>>,
    properties: Option<DepSet<String>>,
    required_use: Option<DepSet<String>>,
    restrict: Option<DepSet<String>>,
    src_uri: Option<DepSet<Uri>>,
    homepage: IndexSet<String>,
    defined_phases: HashSet<String>,
    keywords: IndexSet<String>,
    iuse: IndexSet<String>,
    inherit: IndexSet<String>,
    inherited: IndexSet<String>,
}

macro_rules! split {
    ($s:expr) => {
        $s.split_whitespace().map(String::from).collect()
    };
}

impl Metadata {
    /// Convert raw metadata key value to stored value.
    fn convert(&mut self, eapi: &'static Eapi, key: Key, val: &str) -> crate::Result<()> {
        use crate::depset::parse;
        use Key::*;
        match key {
            Description => self.description = val.to_string(),
            Slot => self.slot = val.to_string(),
            Depend | Bdepend | Idepend | Rdepend | Pdepend => {
                if let Some(val) = parse::pkgdep(val, eapi)
                    .map_err(|e| Error::InvalidValue(format!("invalid {key}: {e}")))?
                {
                    self.deps.insert(key, val);
                }
            }
            License => self.license = parse::license(val)?,
            Properties => self.properties = parse::properties(val)?,
            RequiredUse => self.required_use = parse::required_use(val, eapi)?,
            Restrict => self.restrict = parse::restrict(val)?,
            SrcUri => self.src_uri = parse::src_uri(val, eapi)?,
            Homepage => self.homepage = split!(val),
            DefinedPhases => self.defined_phases = split!(val),
            Keywords => self.keywords = split!(val),
            Iuse => self.iuse = split!(val),
            Inherit => self.inherit = split!(val),
            Inherited => self.inherited = split!(val),
            _ => (),
        }
        Ok(())
    }

    // TODO: use serde to support (de)serializing md5-cache metadata
    fn deserialize(s: &str, eapi: &'static Eapi) -> crate::Result<Self> {
        let mut meta = Metadata::default();
        use crate::depset::parse;
        use Key::*;

        let iter = s
            .lines()
            .filter_map(|l| {
                l.split_once('=').map(|(s, v)| match s {
                    "_eclasses_" => ("INHERITED", v),
                    _ => (s, v),
                })
            })
            .filter_map(|(k, v)| Key::from_str(k).ok().map(|k| (k, v)))
            .filter(|(k, _)| eapi.metadata_keys().contains(k));

        for (key, val) in iter {
            match key {
                Description => meta.description = val.to_string(),
                Slot => meta.slot = val.to_string(),
                Depend | Bdepend | Idepend | Rdepend | Pdepend => {
                    if let Some(val) = parse::pkgdep(val, eapi)
                        .map_err(|e| Error::InvalidValue(format!("invalid {key}: {e}")))?
                    {
                        meta.deps.insert(key, val);
                    }
                }
                License => meta.license = parse::license(val)?,
                Properties => meta.properties = parse::properties(val)?,
                RequiredUse => meta.required_use = parse::required_use(val, eapi)?,
                Restrict => meta.restrict = parse::restrict(val)?,
                SrcUri => meta.src_uri = parse::src_uri(val, eapi)?,
                Homepage => meta.homepage = split!(val),
                DefinedPhases => meta.defined_phases = split!(val),
                Keywords => meta.keywords = split!(val),
                Iuse => meta.iuse = split!(val),
                Inherit => meta.inherit = split!(val),
                Inherited => {
                    meta.inherited = val
                        .split_whitespace()
                        .tuples()
                        .map(|(name, _chksum)| name.to_string())
                        .collect();
                }
                _ => (),
            }
        }

        Ok(meta)
    }

    /// Load metadata from cache.
    pub(crate) fn load(atom: &Atom, eapi: &'static Eapi, repo: &EbuildRepo) -> Option<Self> {
        // TODO: validate cache entries in some fashion?
        let path = build_from_paths!(repo.path(), "metadata", "md5-cache", atom.to_string());
        let s = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                if e.kind() != io::ErrorKind::NotFound {
                    warn!("error loading ebuild metadata: {:?}: {e}", &path);
                }
                return None;
            }
        };

        match Metadata::deserialize(&s, eapi) {
            Ok(m) => Some(m),
            Err(e) => {
                warn!("error deserializing ebuild metadata: {:?}: {e}", &path);
                None
            }
        }
    }

    /// Source ebuild to determine metadata.
    pub(crate) fn source(
        atom: &Atom,
        path: &Utf8Path,
        eapi: &'static Eapi,
        repo: &EbuildRepo,
    ) -> crate::Result<Self> {
        BuildData::update(atom, repo.id());
        // TODO: run sourcing via an external process pool returning the requested variables
        source_ebuild(path)?;
        let mut meta = Metadata::default();

        // verify sourced EAPI matches parsed EAPI
        let sourced_eapi = variables::optional("EAPI");
        let sourced_eapi = sourced_eapi.as_deref().unwrap_or("0");
        if <&Eapi>::from_str(sourced_eapi)? != eapi {
            return Err(Error::InvalidValue(format!(
                "mismatched sourced and parsed EAPIs: {sourced_eapi} != {eapi}"
            )));
        }

        // required metadata variables
        let mut missing = Vec::<&str>::new();
        for key in eapi.mandatory_keys() {
            match key.get(eapi) {
                Some(val) => meta.convert(eapi, *key, &val)?,
                None => missing.push(key.as_ref()),
            }
        }

        if !missing.is_empty() {
            missing.sort();
            let keys = missing.join(", ");
            return Err(Error::InvalidValue(format!("missing required values: {keys}")));
        }

        // metadata variables that default to empty
        for key in eapi.metadata_keys().difference(eapi.mandatory_keys()) {
            if let Some(val) = key.get(eapi) {
                meta.convert(eapi, *key, &val)?;
            }
        }

        // TODO: handle resets in external process pool
        #[cfg(feature = "init")]
        scallop::shell::Shell::reset();

        Ok(meta)
    }

    pub(crate) fn description(&self) -> &str {
        &self.description
    }

    pub(crate) fn slot(&self) -> &str {
        let s = self.slot.as_str();
        s.split_once('/').map_or(s, |x| x.0)
    }

    pub(crate) fn subslot(&self) -> Option<&str> {
        let s = self.slot.as_str();
        s.split_once('/').map(|x| x.1)
    }

    pub(crate) fn deps(&self, key: Key) -> Option<&DepSet<Atom>> {
        self.deps.get(&key)
    }

    pub(crate) fn license(&self) -> Option<&DepSet<String>> {
        self.license.as_ref()
    }

    pub(crate) fn properties(&self) -> Option<&DepSet<String>> {
        self.properties.as_ref()
    }

    pub(crate) fn required_use(&self) -> Option<&DepSet<String>> {
        self.required_use.as_ref()
    }

    pub(crate) fn restrict(&self) -> Option<&DepSet<String>> {
        self.restrict.as_ref()
    }

    pub(crate) fn src_uri(&self) -> Option<&DepSet<Uri>> {
        self.src_uri.as_ref()
    }

    pub(crate) fn homepage(&self) -> &IndexSet<String> {
        &self.homepage
    }

    pub(crate) fn defined_phases(&self) -> &HashSet<String> {
        &self.defined_phases
    }

    pub(crate) fn keywords(&self) -> &IndexSet<String> {
        &self.keywords
    }

    pub(crate) fn iuse(&self) -> &IndexSet<String> {
        &self.iuse
    }

    pub(crate) fn inherit(&self) -> &IndexSet<String> {
        &self.inherit
    }

    pub(crate) fn inherited(&self) -> &IndexSet<String> {
        &self.inherited
    }
}
