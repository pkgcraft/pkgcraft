use std::collections::HashSet;
use std::str::FromStr;
use std::{fs, io};

use camino::Utf8Path;
use indexmap::IndexSet;
use itertools::Itertools;
use scallop::functions;
use scallop::variables::string_value;
use strum::{AsRefStr, Display, EnumString};
use tracing::warn;

use crate::config::Config;
use crate::eapi::Eapi;
use crate::macros::build_from_paths;
use crate::pkgsh::{source_ebuild, BuildData, BUILD_DATA};
use crate::repo::{ebuild::Repo, Repository};
use crate::{atom, Error};

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
            key => string_value(key),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Metadata {
    description: String,
    slot: String,
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
    fn convert(&mut self, key: &Key, val: &str) {
        use Key::*;
        match key {
            Description => self.description = val.to_string(),
            Slot => self.slot = val.to_string(),
            Homepage => self.homepage = split!(val),
            DefinedPhases => self.defined_phases = split!(val),
            Keywords => self.keywords = split!(val),
            Iuse => self.iuse = split!(val),
            Inherit => self.inherit = split!(val),
            Inherited => self.inherited = split!(val),
            _ => (),
        }
    }

    /// Load metadata from cache.
    pub(crate) fn load(atom: &atom::Atom, eapi: &'static Eapi, repo: &Repo) -> Option<Self> {
        // TODO: validate cache entries in some fashion?
        let path = build_from_paths!(repo.path(), "metadata", "md5-cache", atom.to_string());
        match fs::read_to_string(&path) {
            Ok(s) => {
                let mut meta = Metadata::default();
                s.lines()
                    .filter_map(|l| l.split_once('='))
                    .filter_map(|(k, v)| Key::from_str(k).ok().map(|k| (k, v)))
                    .filter(|(k, _)| eapi.metadata_keys().contains(k))
                    .for_each(|(k, v)| meta.convert(&k, v));
                Some(meta)
            }
            Err(e) => {
                if e.kind() != io::ErrorKind::NotFound {
                    warn!("error loading ebuild metadata: {:?}: {e}", &path);
                }
                None
            }
        }
    }

    /// Source ebuild to determine metadata.
    pub(crate) fn source(path: &Utf8Path, eapi: &'static Eapi, repo: &Repo) -> crate::Result<Self> {
        // TODO: rework BuildData handling to drop this hack required by builtins like `inherit`
        let config = Config::current();
        let r = config
            .repos
            .get(repo.id())
            .expect("failed getting repo")
            .as_ebuild()
            .expect("unsupported repo type");
        BUILD_DATA.with(|d| d.borrow_mut().repo = r.clone());

        // TODO: run sourcing via an external process pool returning the requested variables
        source_ebuild(path)?;
        //let mut data = HashMap::new();
        let mut meta = Metadata::default();

        // verify sourced EAPI matches parsed EAPI
        let sourced_eapi = string_value("EAPI");
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
                Some(val) => meta.convert(key, &val),
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
                meta.convert(key, &val);
            }
        }

        // TODO: handle resets in external process pool
        BuildData::reset();
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
