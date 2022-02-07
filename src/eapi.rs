use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
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
type EapiHashSet = HashSet<&'static str>;

#[rustfmt::skip]
static EAPI_OPTIONS: Lazy<EapiOptions> = Lazy::new(|| {
    [
        // EAPI 0

        // RDEPEND=DEPEND if RDEPEND is unset
        ("rdepend_default", true),

        // DESTTREE is exported to the ebuild env
        ("export_desttree", true),

        // INSDESTTREE is exported to the ebuild env
        ("export_insdesttree", true),

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

        // use_with and use_enable support an optional third argument
        ("use_conf_arg", false),

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

        // usev supports an optional second arg
        ("usev_two_args", false),

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
    incremental_keys: EapiHashSet,
    builtins: EapiHashSet,
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

// use the latest EAPI for the Default trait
impl Default for &'static Eapi {
    fn default() -> &'static Eapi {
        EAPI_LATEST
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

    #[inline]
    pub(crate) fn incremental_keys(&self) -> &EapiHashSet {
        &self.incremental_keys
    }

    fn update_options(&self, updates: &[(&'static str, bool)]) -> EapiOptions {
        let mut options = self.options.clone();
        for (key, val) in updates.iter() {
            if options.insert(key, *val).is_none() {
                panic!("option missing default: {:?}", key);
            }
        }
        options
    }

    fn update_keys(&self, updates: &[&'static str]) -> EapiHashSet {
        let mut keys = self.incremental_keys.clone();
        keys.extend(updates);
        keys
    }

    fn update_builtins(&self, add: &[&'static str], remove: &[&'static str]) -> EapiHashSet {
        let mut builtins = self.builtins.clone();
        builtins.extend(add);
        for x in remove {
            if !builtins.remove(x) {
                panic!("unknown builtin: {}", x);
            }
        }
        builtins
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

pub static EAPI0: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "0",
    parent: None,
    options: EAPI_OPTIONS.clone(),
    incremental_keys: ["IUSE", "DEPEND", "RDEPEND", "PDEPEND"]
        .iter()
        .cloned()
        .collect(),
    builtins: [
        "assert",
        "debug-print-function",
        "debug-print",
        "debug-print-section",
        "diropts",
        "die",
        "docinto",
        "dodoc",
        "exeinto",
        "exeopts",
        "EXPORT_FUNCTIONS",
        "has",
        "hasq",
        "hasv",
        "inherit",
        "insinto",
        "insopts",
        "into",
        "libopts",
        "use",
        "useq",
        "usev",
        "use_enable",
        "use_with",
    ]
    .iter()
    .cloned()
    .collect(),
});

pub static EAPI1: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "1",
    parent: Some(&EAPI0),
    options: EAPI0.update_options(&[("slot_deps", true)]),
    incremental_keys: EAPI0.incremental_keys.clone(),
    builtins: EAPI0.builtins.clone(),
});

pub static EAPI2: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "2",
    parent: Some(&EAPI1),
    options: EAPI1.update_options(&[
        ("blockers", true),
        ("use_deps", true),
        ("src_uri_renames", true),
    ]),
    incremental_keys: EAPI1.incremental_keys.clone(),
    builtins: EAPI1.update_builtins(&["default"], &[]),
});

pub static EAPI3: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "3",
    parent: Some(&EAPI2),
    options: EAPI2.options.clone(),
    incremental_keys: EAPI2.incremental_keys.clone(),
    builtins: EAPI2.builtins.clone(),
});

pub static EAPI4: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "4",
    parent: Some(&EAPI3),
    options: EAPI3.update_options(&[
        ("use_dep_defaults", true),
        ("required_use", true),
        ("rdepend_default", false),
        ("use_conf_arg", true),
    ]),
    incremental_keys: EAPI3.update_keys(&["REQUIRED_USE"]),
    builtins: EAPI3.update_builtins(&["docompress", "nonfatal"], &[]),
});

pub static EAPI5: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "5",
    parent: Some(&EAPI4),
    options: EAPI4.update_options(&[
        ("subslots", true),
        ("slot_ops", true),
        ("required_use_one_of", true),
    ]),
    incremental_keys: EAPI4.incremental_keys.clone(),
    builtins: EAPI4.update_builtins(&["usex"], &[]),
});

pub static EAPI6: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "6",
    parent: Some(&EAPI5),
    options: EAPI5.options.clone(),
    incremental_keys: EAPI5.incremental_keys.clone(),
    builtins: EAPI5.update_builtins(&["einstalldocs", "get_libdir", "in_iuse"], &[]),
});

pub static EAPI7: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "7",
    parent: Some(&EAPI6),
    options: EAPI6.update_options(&[("export_desttree", false), ("export_insdesttree", false)]),
    incremental_keys: EAPI6.update_keys(&["BDEPEND"]),
    builtins: EAPI6.update_builtins(&["dostrip", "ver_cut", "ver_rs", "ver_test"], &["libopts"]),
});

pub static EAPI8: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "8",
    parent: Some(&EAPI7),
    options: EAPI7.update_options(&[("src_uri_unrestrict", true), ("usev_two_args", true)]),
    incremental_keys: EAPI7.update_keys(&["IDEPEND", "PROPERTIES", "RESTRICT"]),
    builtins: EAPI7.update_builtins(&[], &["hasq", "hasv", "useq"]),
});

/// Reference to the latest registered EAPI.
pub static EAPI_LATEST: &Lazy<Eapi> = &EAPI8;

/// The latest EAPI with extensions on top.
pub static EAPI_PKGCRAFT: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "pkgcraft",
    parent: Some(EAPI_LATEST),
    options: EAPI_LATEST.update_options(&[("repo_ids", true)]),
    incremental_keys: EAPI_LATEST.incremental_keys.clone(),
    builtins: EAPI_LATEST.builtins.clone(),
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
