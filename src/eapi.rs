use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};

use camino::Utf8Path;
use indexmap::{IndexMap, IndexSet};
use once_cell::sync::{Lazy, OnceCell};
use regex::{escape, Regex, RegexBuilder};
use scallop::builtins::ScopedBuiltins;

use crate::archive::Archive;
use crate::atom::Atom;
use crate::pkgsh::builtins::{parse, BuiltinsMap, BUILTINS_MAP};
use crate::pkgsh::phases::*;
use crate::{Error, Result};

static VALID_EAPI_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^[A-Za-z0-9_][A-Za-z0-9+_.-]*$").unwrap());

type EapiOptions = HashMap<&'static str, bool>;

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

        // support language detection via filename for `doman`
        ("doman_lang_detect", false),

        // SRC_URI -> operator for url filename renaming
        ("src_uri_renames", false),

        // atom use deps -- cat/pkg[use]
        ("use_deps", false),

        // EAPI 4

        // recursive install support via `dodoc -r`
        ("dodoc_recursive", false),

        // support `doman` language override via -i18n option
        ("doman_lang_override", false),

        // atom use defaults -- cat/pkg[use(+)] and cat/pkg[use(-)]
        ("use_dep_defaults", false),

        // REQUIRED_USE support
        ("required_use", false),

        // use_with and use_enable support an optional third argument
        ("use_conf_arg", false),

        // EAPI 5

        // export the running phase name as $EBUILD_PHASE_FUNC
        ("ebuild_phase_func", false),

        // new* helpers can use stdin for content instead of a file
        ("new_supports_stdin", false),

        // running tests in parallel is supported
        ("parallel_tests", false),

        // REQUIRED_USE ?? operator
        ("required_use_one_of", false),

        // atom slot operators -- cat/pkg:=, cat/pkg:*, cat/pkg:0=
        ("slot_ops", false),

        // atom subslots -- cat/pkg:0/4
        ("subslots", false),

        // EAPI 6

        // `die -n` supports nonfatal usage
        ("nonfatal_die", false),

        // failglob shell option is enabled in global scope
        ("global_failglob", false),

        // `unpack` supports absolute and relative paths
        ("unpack_extended_path", false),

        // `unpack` performs case-insensitive file extension matching
        ("unpack_case_insensitive", false),

        // EAPI 8

        // improve insopts/exeopts consistency for install functions
        // https://bugs.gentoo.org/657580
        ("consistent_file_opts", false),

        // relative path support via `dosym -r`
        ("dosym_relative", false),

        // SRC_URI supports fetch+ and mirror+ prefixes
        ("src_uri_unrestrict", false),

        // usev supports an optional second arg
        ("usev_two_args", false),

        // EAPI EXTENDED

        // atom repo deps -- cat/pkg::repo
        ("repo_ids", false),
    ].iter().cloned().collect()
});

type EapiEconfOptions = HashMap<String, (IndexSet<String>, Option<String>)>;

#[derive(Debug, Clone)]
pub struct Eapi {
    id: &'static str,
    parent: Option<&'static Eapi>,
    options: EapiOptions,
    phases: HashMap<String, PhaseFn>,
    incremental_keys: HashSet<String>,
    econf_options: EapiEconfOptions,
    archives: HashSet<String>,
    archives_regex: OnceCell<Regex>,
}

impl Eq for Eapi {}

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
    fn into_eapi(self) -> Result<&'static Eapi>;
}

impl IntoEapi for &'static Eapi {
    fn into_eapi(self) -> Result<&'static Eapi> {
        Ok(self)
    }
}

impl IntoEapi for Option<&str> {
    fn into_eapi(self) -> Result<&'static Eapi> {
        match self {
            None => Ok(&EAPI_PKGCRAFT),
            Some(s) => get_eapi(s),
        }
    }
}

impl IntoEapi for Option<&'static Eapi> {
    fn into_eapi(self) -> Result<&'static Eapi> {
        match self {
            None => Ok(&EAPI_PKGCRAFT),
            Some(eapi) => Ok(eapi),
        }
    }
}

type EconfUpdate<'a> = (&'a str, Option<&'a [&'a str]>, Option<&'a str>);

impl Eapi {
    /// Check if an EAPI has a given feature.
    pub fn has(&self, opt: &str) -> bool {
        match self.options.get(opt) {
            Some(value) => *value,
            None => panic!("unknown EAPI option {opt:?}"),
        }
    }

    /// Parse a package atom using EAPI specific support.
    #[inline]
    pub fn atom<S: AsRef<str>>(&'static self, s: S) -> Result<Atom> {
        Atom::new(s.as_ref(), self)
    }

    pub(crate) fn phases(&self) -> &HashMap<String, PhaseFn> {
        &self.phases
    }

    pub(crate) fn archives_regex(&self) -> &Regex {
        self.archives_regex.get_or_init(|| {
            // Regex matches extensions from the longest to the shortest.
            let mut possible_exts: Vec<String> =
                self.archives.iter().map(|s| escape(s.as_str())).collect();
            possible_exts.sort_by_cached_key(|s| s.len());
            possible_exts.reverse();
            RegexBuilder::new(&format!(r"\.(?P<ext>{})$", possible_exts.join("|")))
                .case_insensitive(self.has("unpack_case_insensitive"))
                .build()
                .unwrap()
        })
    }

    pub(crate) fn archive_from_path<P>(&self, path: P) -> Result<(String, Archive)>
    where
        P: AsRef<Utf8Path>,
    {
        let path = path.as_ref();

        match self.archives_regex().captures(path.as_str()) {
            Some(c) => {
                let ext = String::from(c.name("ext").unwrap().as_str());
                let archive = Archive::from_path(path)?;
                Ok((ext, archive))
            }
            None => Err(Error::InvalidValue(format!("unknown archive format: {path:?}"))),
        }
    }

    pub(crate) fn builtins<S: AsRef<str>>(&self, scope: S) -> Result<&BuiltinsMap> {
        let scope = scope.as_ref();
        BUILTINS_MAP
            .get(self)
            .unwrap()
            .get(scope)
            .ok_or_else(|| Error::Eapi(format!("EAPI {}, unknown scope: {scope}", self.id)))
    }

    pub(crate) fn scoped_builtins<S: AsRef<str>>(&self, scope: S) -> Result<ScopedBuiltins> {
        let builtins: Vec<&str> = self.builtins(scope)?.keys().copied().collect();
        Ok(ScopedBuiltins::new((&builtins, &[]))?)
    }

    pub(crate) fn incremental_keys(&self) -> &HashSet<String> {
        &self.incremental_keys
    }

    pub(crate) fn econf_options(&self) -> &EapiEconfOptions {
        &self.econf_options
    }

    fn update_options(&self, updates: &[(&'static str, bool)]) -> EapiOptions {
        let mut options = self.options.clone();
        for (key, val) in updates.iter() {
            if options.insert(key, *val).is_none() {
                panic!("option missing default: {key:?}");
            }
        }
        options
    }

    fn update_phases(&self, updates: &[(&str, PhaseFn)]) -> HashMap<String, PhaseFn> {
        let mut phases = self.phases.clone();
        phases.extend(updates.iter().map(|(s, f)| (s.to_string(), *f)));
        phases
    }

    fn update_keys(&self, updates: &[&'static str]) -> HashSet<String> {
        let mut keys = self.incremental_keys.clone();
        keys.extend(updates.iter().map(|s| s.to_string()));
        keys
    }

    fn update_econf(&self, updates: &[EconfUpdate]) -> EapiEconfOptions {
        let mut econf_options = self.econf_options.clone();
        for (opt, markers, val) in updates {
            let markers = markers
                .unwrap_or(&[opt])
                .iter()
                .map(|s| s.to_string())
                .collect();
            let val = val.map(|s| s.to_string());
            econf_options.insert(opt.to_string(), (markers, val));
        }
        econf_options
    }

    fn update_archives(&self, add: &[&str], remove: &[&str]) -> HashSet<String> {
        let mut archives = self.archives.clone();
        archives.extend(add.iter().map(|s| s.to_string()));
        for x in remove {
            if !archives.remove(*x) {
                panic!("disabling unknown archive format: {x:?}");
            }
        }
        archives
    }
}

impl fmt::Display for Eapi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// Get a EAPI given its identifier.
pub fn get_eapi(id: &str) -> Result<&'static Eapi> {
    match KNOWN_EAPIS.get(id) {
        Some(eapi) => Ok(eapi),
        None => match VALID_EAPI_RE.is_match(id) {
            true => Err(Error::Eapi(format!("unknown EAPI: {id:?}"))),
            false => Err(Error::Eapi(format!("invalid EAPI: {id:?}"))),
        },
    }
}

pub static EAPI0: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "0",
    parent: None,
    options: EAPI_OPTIONS.clone(),
    phases: [
        ("pkg_setup", phase_stub as PhaseFn),
        ("pkg_config", phase_stub as PhaseFn),
        ("pkg_info", phase_stub as PhaseFn),
        ("pkg_nofetch", phase_stub as PhaseFn),
        ("pkg_prerm", phase_stub as PhaseFn),
        ("pkg_postrm", phase_stub as PhaseFn),
        ("pkg_preinst", phase_stub as PhaseFn),
        ("pkg_postinst", phase_stub as PhaseFn),
        ("src_unpack", eapi0::src_unpack as PhaseFn),
        ("src_compile", eapi0::src_compile as PhaseFn),
        ("src_test", eapi0::src_test as PhaseFn),
        ("src_install", phase_stub as PhaseFn),
    ]
    .iter()
    .map(|(s, f)| (s.to_string(), *f))
    .collect(),
    incremental_keys: ["IUSE", "DEPEND", "RDEPEND", "PDEPEND"]
        .iter()
        .map(|s| s.to_string())
        .collect(),
    econf_options: EapiEconfOptions::new(),
    #[rustfmt::skip]
    archives: [
        "tar",
        "gz", "Z",
        "tar.gz", "tgz", "tar.Z",
        "bz2", "bz",
        "tar.bz2", "tbz2", "tar.bz", "tbz",
        "zip", "ZIP", "jar",
        "7z", "7Z",
        "rar", "RAR",
        "LHA", "LHa", "lha", "lzh",
        "a",
        "deb",
        "lzma",
        "tar.lzma",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect(),
    archives_regex: OnceCell::new(),
});

pub static EAPI1: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "1",
    parent: Some(&EAPI0),
    options: EAPI0.update_options(&[("slot_deps", true)]),
    phases: EAPI0.update_phases(&[("src_compile", eapi1::src_compile as PhaseFn)]),
    incremental_keys: EAPI0.incremental_keys.clone(),
    econf_options: EAPI0.econf_options.clone(),
    archives: EAPI0.archives.clone(),
    archives_regex: OnceCell::new(),
});

pub static EAPI2: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "2",
    parent: Some(&EAPI1),
    options: EAPI1.update_options(&[
        ("blockers", true),
        ("doman_lang_detect", true),
        ("use_deps", true),
        ("src_uri_renames", true),
    ]),
    phases: EAPI1.update_phases(&[
        ("src_prepare", phase_stub as PhaseFn),
        ("src_compile", eapi2::src_compile as PhaseFn),
        ("src_configure", eapi2::src_configure as PhaseFn),
    ]),
    incremental_keys: EAPI1.incremental_keys.clone(),
    econf_options: EAPI1.econf_options.clone(),
    archives: EAPI1.archives.clone(),
    archives_regex: OnceCell::new(),
});

pub static EAPI3: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "3",
    parent: Some(&EAPI2),
    options: EAPI2.options.clone(),
    phases: EAPI2.phases.clone(),
    incremental_keys: EAPI2.incremental_keys.clone(),
    econf_options: EAPI2.econf_options.clone(),
    archives: EAPI2.update_archives(&["tar.xz", "xz"], &[]),
    archives_regex: OnceCell::new(),
});

pub static EAPI4: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "4",
    parent: Some(&EAPI3),
    options: EAPI3.update_options(&[
        ("dodoc_recursive", true),
        ("doman_lang_override", true),
        ("rdepend_default", false),
        ("required_use", true),
        ("use_conf_arg", true),
        ("use_dep_defaults", true),
    ]),
    phases: EAPI3.update_phases(&[
        ("pkg_pretend", phase_stub as PhaseFn),
        ("src_install", eapi4::src_install as PhaseFn),
    ]),
    incremental_keys: EAPI3.update_keys(&["REQUIRED_USE"]),
    econf_options: EAPI3.update_econf(&[("--disable-dependency-tracking", None, None)]),
    archives: EAPI3.archives.clone(),
    archives_regex: OnceCell::new(),
});

pub static EAPI5: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "5",
    parent: Some(&EAPI4),
    options: EAPI4.update_options(&[
        ("ebuild_phase_func", true),
        ("new_supports_stdin", true),
        ("parallel_tests", true),
        ("required_use_one_of", true),
        ("slot_ops", true),
        ("subslots", true),
    ]),
    phases: EAPI4.phases.clone(),
    incremental_keys: EAPI4.incremental_keys.clone(),
    econf_options: EAPI4.update_econf(&[("--disable-silent-rules", None, None)]),
    archives: EAPI4.archives.clone(),
    archives_regex: OnceCell::new(),
});

pub static EAPI6: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "6",
    parent: Some(&EAPI5),
    options: EAPI5.update_options(&[
        ("nonfatal_die", true),
        ("global_failglob", true),
        ("unpack_extended_path", true),
        ("unpack_case_insensitive", true),
    ]),
    phases: EAPI5.update_phases(&[
        ("src_prepare", eapi6::src_prepare as PhaseFn),
        ("src_install", eapi6::src_install as PhaseFn),
    ]),
    incremental_keys: EAPI5.incremental_keys.clone(),
    econf_options: EAPI5.update_econf(&[
        ("--docdir", None, Some("${EPREFIX}/usr/share/doc/${PF}")),
        ("--htmldir", None, Some("${EPREFIX}/usr/share/doc/${PF}/html")),
    ]),
    archives: EAPI5.update_archives(&["txz"], &[]),
    archives_regex: OnceCell::new(),
});

pub static EAPI7: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "7",
    parent: Some(&EAPI6),
    options: EAPI6.update_options(&[("export_desttree", false), ("export_insdesttree", false)]),
    phases: EAPI6.phases.clone(),
    incremental_keys: EAPI6.update_keys(&["BDEPEND"]),
    econf_options: EAPI6.update_econf(&[("--with-sysroot", None, Some("${ESYSROOT:-/}"))]),
    archives: EAPI6.archives.clone(),
    archives_regex: OnceCell::new(),
});

pub static EAPI8: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "8",
    parent: Some(&EAPI7),
    options: EAPI7.update_options(&[
        ("consistent_file_opts", true),
        ("dosym_relative", true),
        ("src_uri_unrestrict", true),
        ("usev_two_args", true),
    ]),
    phases: EAPI7.phases.clone(),
    incremental_keys: EAPI7.update_keys(&["IDEPEND", "PROPERTIES", "RESTRICT"]),
    econf_options: EAPI7.update_econf(&[
        ("--datarootdir", None, Some("${EPREFIX}/usr/share")),
        ("--disable-static", Some(&["--disable-static", "--enable-static"]), None),
    ]),
    #[rustfmt::skip]
    archives: EAPI7.update_archives(&[], &[
        "7z", "7Z",
        "rar", "RAR",
        "LHA", "LHa", "lha", "lzh",
    ]),
    archives_regex: OnceCell::new(),
});

/// Reference to the latest registered EAPI.
pub static EAPI_LATEST: &Lazy<Eapi> = &EAPI8;

/// The latest EAPI with extensions on top.
pub static EAPI_PKGCRAFT: Lazy<Eapi> = Lazy::new(|| Eapi {
    id: "pkgcraft",
    parent: Some(EAPI_LATEST),
    options: EAPI_LATEST.update_options(&[("repo_ids", true)]),
    phases: EAPI_LATEST.phases.clone(),
    incremental_keys: EAPI_LATEST.incremental_keys.clone(),
    econf_options: EAPI_LATEST.econf_options.clone(),
    archives: EAPI_LATEST.archives.clone(),
    archives_regex: OnceCell::new(),
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

pub(crate) fn supported<S: AsRef<str>>(val: S) -> Result<IndexSet<&'static Eapi>> {
    let (start, end) = parse::range(val.as_ref(), KNOWN_EAPIS.len() - 1)?;
    Ok((start..=end).map(|n| KNOWN_EAPIS[n]).collect())
}

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
            assert_eq!(format!("{eapi}"), format!("{id}"));
        }
    }

    #[test]
    fn test_atom_parsing() {
        let mut atom;
        atom = EAPI0.atom("cat/pkg").unwrap();
        assert_eq!(atom.category, "cat");
        assert_eq!(atom.package, "pkg");
        assert_eq!(format!("{atom}"), "cat/pkg");

        atom = EAPI1.atom("cat/pkg:0").unwrap();
        assert_eq!(atom.category, "cat");
        assert_eq!(atom.package, "pkg");
        assert_eq!(atom.slot.as_ref().unwrap(), "0");
        assert_eq!(format!("{atom}"), "cat/pkg:0");
    }
}
