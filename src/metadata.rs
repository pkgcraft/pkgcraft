use std::str::FromStr;
use std::{fs, io};

use camino::Utf8Path;
use indexmap::IndexSet;
use scallop::functions;
use scallop::variables::string_value;
use strum::{AsRefStr, Display, EnumString};
use tracing::warn;

use crate::config::Config;
use crate::macros::build_from_paths;
use crate::pkgsh::{source_ebuild, BUILD_DATA};
use crate::repo::{ebuild::Repo, Repository};
use crate::{atom, eapi, Error};

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
    pub(crate) fn get(&self, eapi: &'static eapi::Eapi) -> Option<String> {
        match self {
            Key::DefinedPhases => {
                let mut phase_names = vec![];
                for phase in eapi.phases() {
                    if functions::find(phase).is_some() {
                        phase_names.push(phase.short_name());
                    }
                }
                match phase_names.is_empty() {
                    true => None,
                    false => {
                        phase_names.sort_unstable();
                        Some(phase_names.join(" "))
                    }
                }
            }
            key => string_value(key),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Metadata {
    description: String,
    fullslot: String,
    homepage: Vec<String>,
    keywords: IndexSet<String>,
    iuse: IndexSet<String>,
    inherit: IndexSet<String>,
    inherited: IndexSet<String>,
}

impl Metadata {
    /// Convert raw metadata key value to stored value.
    fn convert(&mut self, key: &Key, val: String) {
        use Key::*;
        match key {
            Description => self.description = val,
            Slot => self.fullslot = val,
            Homepage => self.homepage = val.split_whitespace().map(String::from).collect(),
            Keywords => self.keywords = val.split_whitespace().map(String::from).collect(),
            Iuse => self.iuse = val.split_whitespace().map(String::from).collect(),
            Inherit => self.inherit = val.split_whitespace().map(String::from).collect(),
            Inherited => self.inherited = val.split_whitespace().map(String::from).collect(),
            _ => (),
        }
    }

    /// Load metadata from cache.
    pub(crate) fn load(atom: &atom::Atom, eapi: &'static eapi::Eapi, repo: &Repo) -> Option<Self> {
        // TODO: validate cache entries in some fashion?
        let path = build_from_paths!(repo.path(), "metadata", "md5-cache", atom.to_string());
        match fs::read_to_string(&path) {
            Ok(s) => {
                let mut meta = Metadata::default();
                s.lines()
                    .filter_map(|l| l.split_once('='))
                    .filter_map(|(k, v)| Key::from_str(k).ok().map(|k| (k, v)))
                    .filter(|(k, _)| eapi.metadata_keys().contains(k))
                    .for_each(|(k, v)| meta.convert(&k, v.to_string()));
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
    pub(crate) fn source(
        path: &Utf8Path,
        eapi: &'static eapi::Eapi,
        repo: &Repo,
    ) -> crate::Result<Self> {
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
        if eapi::get_eapi(&sourced_eapi)? != eapi {
            return Err(Error::InvalidValue(format!(
                "mismatched sourced and parsed EAPIs: {sourced_eapi} != {eapi}"
            )));
        }

        // required metadata variables
        let mut missing = Vec::<&str>::new();
        for key in eapi.mandatory_keys() {
            match key.get(eapi) {
                Some(val) => meta.convert(key, val),
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
                meta.convert(key, val);
            }
        }

        Ok(meta)
    }

    pub(crate) fn description(&self) -> &str {
        &self.description
    }

    pub(crate) fn slot(&self) -> &str {
        let s = self.fullslot.as_str();
        s.split_once('/').map_or(s, |x| x.0)
    }

    pub(crate) fn subslot(&self) -> &str {
        let s = self.fullslot.as_str();
        s.split_once('/').map_or(s, |x| x.1)
    }

    pub(crate) fn homepage(&self) -> &[String] {
        &self.homepage
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
