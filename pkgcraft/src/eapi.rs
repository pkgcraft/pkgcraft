use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

use indexmap::IndexMap;
// TODO: use std implementation if it becomes available
// https://github.com/rust-lang/rust/issues/74465
use once_cell::sync::Lazy;
use regex::Regex;

use crate::atom;
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

#[derive(Debug, PartialEq)]
pub struct Eapi {
    id: &'static str,
    parent: Option<&'static Eapi>,
    options: EapiOptions,
}

impl PartialOrd for Eapi {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let eapi_num = KNOWN_EAPIS.get_index_of(&self.id).unwrap();
        let other_num = KNOWN_EAPIS.get_index_of(&other.id).unwrap();
        let ordering = eapi_num.partial_cmp(&other_num);
        // invert ordering since KNOWN_EAPIS starts with the most recent EAPI
        match ordering {
            Some(Ordering::Less) => Some(Ordering::Greater),
            Some(Ordering::Greater) => Some(Ordering::Less),
            _ => ordering,
        }
    }
}

impl Eapi {
    pub fn has(&self, opt: &str) -> bool {
        match self.options.get(opt) {
            Some(value) => *value,
            None => panic!("unknown EAPI option {:?}", opt),
        }
    }

    pub fn atom(&'static self, s: &str) -> Result<atom::Atom, atom::ParseError> {
        atom::parse::dep(s, self)
    }

    fn new(
        id: &'static str,
        parent: Option<&'static Eapi>,
        eapi_options: Option<&EapiOptions>,
    ) -> Eapi {
        // clone inherited options
        let mut options: EapiOptions = match parent {
            Some(x) => x.options.clone(),
            None => EAPI_OPTIONS.clone(),
        };

        // merge EAPI specific options
        if let Some(x) = eapi_options {
            options.extend(x);
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

pub fn get_eapi(id: &str) -> Result<&'static Eapi, Error> {
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

pub static EAPI_LATEST: &Lazy<Eapi> = &EAPI8;

#[rustfmt::skip]
pub static EAPI_EXTENDED: Lazy<Eapi> = Lazy::new(|| {
    let options: EapiOptions = [
        ("repo_ids", true),
    ].iter().cloned().collect();
    Eapi::new("extended", Some(EAPI_LATEST), Some(&options))
});

pub static KNOWN_EAPIS: Lazy<IndexMap<&'static str, &'static Eapi>> = Lazy::new(|| {
    let mut eapis: IndexMap<&'static str, &'static Eapi> = IndexMap::new();
    let mut eapi: &Eapi = EAPI_LATEST;
    while let Some(x) = eapi.parent {
        eapis.insert(eapi.id, eapi);
        eapi = x;
    }
    eapis.insert(eapi.id, eapi);
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
