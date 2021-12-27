use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::atom::Atom;
use crate::error::Error;

static VALID_EAPI_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^[A-Za-z0-9_][A-Za-z0-9+_.-]*$").unwrap());

type EapiOptions = HashMap<&'static str, bool>;

#[rustfmt::skip]
static EAPI_OPTIONS: Lazy<EapiOptions> = Lazy::new(|| {
    [
        // EAPI 1

        // IUSE defaults
        ("iuse_defaults", false),

        // atom slot deps -- cat/pkg:0
        ("slot_deps", false),

        // EAPI 2

        // atom blockers -- !cat/pkg and !!cat/pkg
        ("blockers", false),

        // atom use deps -- cat/pkg[use]
        ("use_deps", false),

        // SRC_URI -> operator for url filename renaming
        ("src_uri_renames", false),

        // EAPI 4

        // atom use defaults -- cat/pkg[use(+)] and cat/pkg[use(-)]
        ("use_dep_defaults", false),

        // REQUIRED_USE support
        ("required_use", false),

        // EAPI 5

        // atom subslots -- cat/pkg:0/4
        ("subslots", false),

        // atom slot operators -- cat/pkg:=, cat/pkg:*, cat/pkg:0=
        ("slot_ops", false),

        // REQUIRED_USE ?? operator
        ("required_use_one_of", false),

        // EAPI 8

        // SRC_URI supports fetch+ and mirror+ prefixes
        ("src_uri_unrestrict", false),

        // EAPI EXTENDED

        // atom repo deps -- cat/pkg::repo
        ("repo_ids", false),
    ].iter().cloned().collect()
});

#[derive(Debug, Eq, Clone)]
pub struct Eapi {
    id: &'static str,
    parent: Option<&'static Eapi>,
    options: EapiOptions,
}

impl PartialEq for Eapi {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Hash for Eapi {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialOrd for Eapi {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let self_index = KNOWN_EAPIS.get_index_of(&self.id).unwrap();
        let other_index = KNOWN_EAPIS.get_index_of(&other.id).unwrap();
        self_index.partial_cmp(&other_index)
    }
}

pub trait IntoEapi {
    fn into_eapi(self) -> crate::Result<&'static Eapi>;
}

impl IntoEapi for &'static Eapi {
    fn into_eapi(self) -> crate::Result<&'static Eapi> {
        Ok(self)
    }
}

impl IntoEapi for Option<&str> {
    fn into_eapi(self) -> crate::Result<&'static Eapi> {
        match self {
            None => Ok(&EAPI_PKGCRAFT),
            Some(s) => get_eapi(s),
        }
    }
}

impl IntoEapi for Option<&'static Eapi> {
    fn into_eapi(self) -> crate::Result<&'static Eapi> {
        match self {
            None => Ok(&EAPI_PKGCRAFT),
            Some(eapi) => Ok(eapi),
        }
    }
}

impl Eapi {
    /// Check if an EAPI has a given feature.
    pub fn has(&self, opt: &str) -> bool {
        match self.options.get(opt) {
            Some(value) => *value,
            None => panic!("unknown EAPI option {:?}", opt),
        }
    }

    /// Parse a package atom using EAPI specific support.
    #[inline]
    pub fn atom<S: AsRef<str>>(&'static self, s: S) -> crate::Result<Atom> {
        Atom::new(s.as_ref(), self)
    }

    fn new(
        id: &'static str,
        parent: Option<&'static Eapi>,
        eapi_options: Option<&EapiOptions>,
    ) -> Eapi {
        // clone inherited options
        let mut options = match parent {
            Some(x) => x.options.clone(),
            None => EAPI_OPTIONS.clone(),
        };

        // merge EAPI specific options while verifying defaults exist
        if let Some(map) = eapi_options {
            for (key, val) in map.iter() {
                if options.insert(key, *val).is_none() {
                    panic!("EAPI {:?} option missing default: {:?}", id, key);
                }
            }
        }

        Eapi {
            id,
            parent,
            options,
        }
    }
}

impl fmt::Display for Eapi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// Get a EAPI given its identifier.
pub fn get_eapi(id: &str) -> crate::Result<&'static Eapi> {
    match KNOWN_EAPIS.get(id) {
        Some(eapi) => Ok(eapi),
        None => match VALID_EAPI_RE.is_match(id) {
            true => Err(Error::Eapi(format!("unknown EAPI: {:?}", id))),
            false => Err(Error::Eapi(format!("invalid EAPI: {:?}", id))),
        },
    }
}

#[rustfmt::skip]
pub static EAPI0: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("0", None, None)
});

#[rustfmt::skip]
pub static EAPI1: Lazy<Eapi> = Lazy::new(|| {
    let options: EapiOptions = [
        ("slot_deps", true),
    ].iter().cloned().collect();
    Eapi::new("1", Some(&EAPI0), Some(&options))
});

#[rustfmt::skip]
pub static EAPI2: Lazy<Eapi> = Lazy::new(|| {
    let options: EapiOptions = [
        ("blockers", true),
        ("use_deps", true),
        ("src_uri_renames", true),
    ].iter().cloned().collect();
    Eapi::new("2", Some(&EAPI1), Some(&options))
});

#[rustfmt::skip]
pub static EAPI3: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("3", Some(&EAPI2), None)
});

#[rustfmt::skip]
pub static EAPI4: Lazy<Eapi> = Lazy::new(|| {
    let options: EapiOptions = [
        ("use_dep_defaults", true),
        ("required_use", true),
    ].iter().cloned().collect();
    Eapi::new("4", Some(&EAPI3), Some(&options))
});

#[rustfmt::skip]
pub static EAPI5: Lazy<Eapi> = Lazy::new(|| {
    let options: EapiOptions = [
        ("subslots", true),
        ("slot_ops", true),
        ("required_use_one_of", true),
    ].iter().cloned().collect();
    Eapi::new("5", Some(&EAPI4), Some(&options))
});

#[rustfmt::skip]
pub static EAPI6: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("6", Some(&EAPI5), None)
});

#[rustfmt::skip]
pub static EAPI7: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("7", Some(&EAPI6), None)
});

#[rustfmt::skip]
pub static EAPI8: Lazy<Eapi> = Lazy::new(|| {
    let options: EapiOptions = [
        ("src_uri_unrestrict", true),
    ].iter().cloned().collect();
    Eapi::new("8", Some(&EAPI7), Some(&options))
});

/// Reference to the latest registered EAPI.
pub static EAPI_LATEST: &Lazy<Eapi> = &EAPI8;

/// The latest EAPI with extensions on top.
#[rustfmt::skip]
pub static EAPI_PKGCRAFT: Lazy<Eapi> = Lazy::new(|| {
    let options: EapiOptions = [
        ("repo_ids", true),
    ].iter().cloned().collect();
    Eapi::new("pkgcraft", Some(EAPI_LATEST), Some(&options))
});

/// Ordered mapping of official EAPI identifiers to instances.
pub static OFFICIAL_EAPIS: Lazy<IndexMap<&'static str, &'static Eapi>> = Lazy::new(|| {
    let mut eapis: IndexMap<&'static str, &'static Eapi> = IndexMap::new();
    let mut eapi: &Eapi = EAPI_LATEST;
    while let Some(x) = eapi.parent {
        eapis.insert(eapi.id, eapi);
        eapi = x;
    }
    eapis.insert(eapi.id, eapi);
    // reverse so it's in chronological order
    eapis.reverse();
    eapis
});

/// Ordered mapping of known EAPI identifiers to instances.
pub static KNOWN_EAPIS: Lazy<IndexMap<&'static str, &'static Eapi>> = Lazy::new(|| {
    let mut eapis = OFFICIAL_EAPIS.clone();
    eapis.insert(EAPI_PKGCRAFT.id, &EAPI_PKGCRAFT);
    eapis
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_eapi() {
        assert!(get_eapi("-invalid").is_err());
        assert!(get_eapi("unknown").is_err());
        assert_eq!(*get_eapi("8").unwrap(), *EAPI8);
    }

    #[test]
    fn test_ordering() {
        assert!(*EAPI0 < **EAPI_LATEST);
        assert!(*EAPI0 <= *EAPI0);
        assert!(*EAPI0 == *EAPI0);
        assert!(*EAPI0 >= *EAPI0);
        assert!(**EAPI_LATEST > *EAPI0);
    }

    #[test]
    fn test_has() {
        assert!(!EAPI0.has("use_deps"));
        assert!(EAPI_LATEST.has("use_deps"));
    }

    #[test]
    #[should_panic(expected = "unknown EAPI option \"unknown\"")]
    fn test_has_unknown() {
        EAPI_LATEST.has("unknown");
    }

    #[test]
    fn test_fmt() {
        for (id, eapi) in KNOWN_EAPIS.iter() {
            assert_eq!(format!("{}", eapi), format!("{}", id));
        }
    }

    #[test]
    fn test_atom_parsing() {
        let mut atom;
        atom = EAPI0.atom("cat/pkg").unwrap();
        assert_eq!(atom.category, "cat");
        assert_eq!(atom.package, "pkg");
        assert_eq!(format!("{}", atom), "cat/pkg");

        atom = EAPI1.atom("cat/pkg:0").unwrap();
        assert_eq!(atom.category, "cat");
        assert_eq!(atom.package, "pkg");
        assert_eq!(atom.slot.as_ref().unwrap(), "0");
        assert_eq!(format!("{}", atom), "cat/pkg:0");
    }
}
