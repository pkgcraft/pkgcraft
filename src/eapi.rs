use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

use crate::atom;

use indexmap::IndexMap;
// TODO: use std implementation if it becomes available
// https://github.com/rust-lang/rust/issues/74465
use once_cell::sync::Lazy;

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
    options: EapiOptions,
}

// Technically EAPIs can't be ordered in this fashion since arbitrary strings are allowed for
// names, in those cases None is returned since comparison isn't possible.
impl PartialOrd for Eapi {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let eapi_num: u32 = self.id.parse().ok()?;
        let other_num: u32 = other.id.parse().ok()?;
        eapi_num.partial_cmp(&other_num)
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
        atom::parse(s, &self)
    }

    fn register(
        id: &'static str,
        parent: Option<&'static Eapi>,
        options: Option<&EapiOptions>,
    ) -> Eapi {
        // clone inherited options
        let mut eapi_options: EapiOptions = match parent {
            Some(x) => x.options.clone(),
            None => EAPI_OPTIONS.clone(),
        };

        // merge EAPI specific options
        match options {
            Some(x) => eapi_options.extend(x),
            None => (),
        };

        Eapi {
            id: id,
            options: eapi_options,
        }
    }
}

impl fmt::Display for Eapi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "EAPI {}", self.id)
    }
}

// TODO: use different error for invalid EAPIs
pub fn get_eapi(id: &str) -> Result<&'static Eapi, &'static str> {
    match KNOWN_EAPIS.get(id) {
        Some(eapi) => Ok(eapi),
        None => Err("unknown EAPI"),
    }
}

#[rustfmt::skip]
pub static EAPI0: Lazy<Eapi> = Lazy::new(|| {
    Eapi::register("0", None, None)
});

#[rustfmt::skip]
pub static EAPI1: Lazy<Eapi> = Lazy::new(|| {
    let options: EapiOptions = [
        ("slot_deps", true),
    ].iter().cloned().collect();
    Eapi::register("1", Some(&EAPI0), Some(&options))
});

#[rustfmt::skip]
pub static EAPI2: Lazy<Eapi> = Lazy::new(|| {
    let options: EapiOptions = [
        ("blockers", true),
        ("use_deps", true),
        ("src_uri_renames", true),
    ].iter().cloned().collect();
    Eapi::register("2", Some(&EAPI1), Some(&options))
});

#[rustfmt::skip]
pub static EAPI3: Lazy<Eapi> = Lazy::new(|| {
    Eapi::register("3", Some(&EAPI2), None)
});

#[rustfmt::skip]
pub static EAPI4: Lazy<Eapi> = Lazy::new(|| {
    let options: EapiOptions = [
        ("use_dep_defaults", true),
        ("required_use", true),
    ].iter().cloned().collect();
    Eapi::register("4", Some(&EAPI3), Some(&options))
});

#[rustfmt::skip]
pub static EAPI5: Lazy<Eapi> = Lazy::new(|| {
    let options: EapiOptions = [
        ("subslots", true),
        ("slot_ops", true),
        ("required_use_one_of", true),
    ].iter().cloned().collect();
    Eapi::register("5", Some(&EAPI4), Some(&options))
});

#[rustfmt::skip]
pub static EAPI6: Lazy<Eapi> = Lazy::new(|| {
    Eapi::register("6", Some(&EAPI5), None)
});

#[rustfmt::skip]
pub static EAPI7: Lazy<Eapi> = Lazy::new(|| {
    Eapi::register("7", Some(&EAPI6), None)
});

#[rustfmt::skip]
pub static EAPI8: Lazy<Eapi> = Lazy::new(|| {
    let options: EapiOptions = [
        ("src_uri_unrestrict", true),
    ].iter().cloned().collect();
    Eapi::register("8", Some(&EAPI7), Some(&options))
});

pub static EAPI_LATEST: &Lazy<Eapi> = &EAPI8;

#[rustfmt::skip]
pub static EAPI_EXTENDED: Lazy<Eapi> = Lazy::new(|| {
    let options: EapiOptions = [
        ("repo_ids", true),
    ].iter().cloned().collect();
    Eapi::register("extended", Some(EAPI_LATEST), Some(&options))
});

pub static KNOWN_EAPIS: Lazy<IndexMap<&'static str, &'static Eapi>> = Lazy::new(|| {
    let mut eapis: IndexMap<&'static str, &'static Eapi> = IndexMap::new();
    eapis.insert("0", &EAPI0);
    eapis.insert("1", &EAPI1);
    eapis.insert("2", &EAPI2);
    eapis.insert("3", &EAPI3);
    eapis.insert("4", &EAPI4);
    eapis.insert("5", &EAPI5);
    eapis.insert("6", &EAPI6);
    eapis.insert("7", &EAPI7);
    eapis.insert("8", &EAPI8);
    eapis
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_eapi() {
        assert!(get_eapi("unknown").is_err());
        assert_eq!(*get_eapi("8").unwrap(), *EAPI8);
    }

    #[test]
    fn latest() {
        assert!(**EAPI_LATEST == **KNOWN_EAPIS.last().unwrap().1);
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
