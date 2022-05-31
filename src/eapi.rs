use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};

use camino::Utf8Path;
use indexmap::{IndexMap, IndexSet};
use once_cell::sync::{Lazy, OnceCell};
use regex::{escape, Regex, RegexBuilder};
use scallop::builtins::ScopedBuiltins;
use scallop::functions;
use scallop::variables::string_value;
use strum::{AsRefStr, Display};

use crate::archive::Archive;
use crate::atom::Atom;
use crate::pkgsh::builtins::{parse, BuiltinsMap, BUILTINS_MAP};
use crate::pkgsh::phase::*;
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
    ].into_iter().collect()
});

type EapiEconfOptions = HashMap<&'static str, (IndexSet<String>, Option<String>)>;

#[derive(AsRefStr, Display, Debug, PartialEq, Eq, Hash, Copy, Clone)]
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

use Key::*;
impl Key {
    pub(crate) fn get(&self, eapi: &'static Eapi) -> Option<String> {
        match self {
            DefinedPhases => {
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
pub struct Eapi {
    id: &'static str,
    parent: Option<&'static Eapi>,
    options: EapiOptions,
    phases: HashSet<Phase>,
    dep_keys: HashSet<Key>,
    incremental_keys: HashSet<Key>,
    mandatory_keys: HashSet<Key>,
    metadata_keys: HashSet<Key>,
    econf_options: EapiEconfOptions,
    archives: HashSet<&'static str>,
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
        let self_index = EAPIS.get_index_of(&self.id).unwrap();
        let other_index = EAPIS.get_index_of(&other.id).unwrap();
        self_index.partial_cmp(&other_index)
    }
}

// use the latest EAPI for the Default trait
impl Default for &'static Eapi {
    fn default() -> &'static Eapi {
        &EAPI_LATEST
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

impl IntoEapi for &str {
    fn into_eapi(self) -> Result<&'static Eapi> {
        get_eapi(self)
    }
}

impl IntoEapi for Option<&str> {
    fn into_eapi(self) -> Result<&'static Eapi> {
        match self {
            None => Ok(Default::default()),
            Some(s) => get_eapi(s),
        }
    }
}

impl IntoEapi for Option<&'static Eapi> {
    fn into_eapi(self) -> Result<&'static Eapi> {
        match self {
            None => Ok(Default::default()),
            Some(eapi) => Ok(eapi),
        }
    }
}

type EconfUpdate<'a> = (&'static str, Option<&'a [&'a str]>, Option<&'a str>);

impl Eapi {
    fn new(id: &'static str, parent: Option<&'static Eapi>) -> Eapi {
        let mut eapi = match parent {
            Some(e) => e.clone(),
            None => Eapi {
                options: EAPI_OPTIONS.clone(),
                ..Default::default()
            },
        };
        eapi.id = id;
        eapi.parent = parent;
        eapi
    }

    /// Return the EAPI's identifier.
    pub fn as_str(&self) -> &str {
        self.id
    }

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

    pub(crate) fn phases(&self) -> &HashSet<Phase> {
        &self.phases
    }

    pub(crate) fn archives_regex(&self) -> &Regex {
        self.archives_regex.get_or_init(|| {
            // Regex matches extensions from the longest to the shortest.
            let mut possible_exts: Vec<String> = self.archives.iter().map(|s| escape(s)).collect();
            possible_exts.sort_by_key(|s| s.len());
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

    /// Metadata variables for dependencies.
    pub fn dep_keys(&self) -> &HashSet<Key> {
        &self.dep_keys
    }

    /// Metadata variables that are incrementally handled.
    pub(crate) fn incremental_keys(&self) -> &HashSet<Key> {
        &self.incremental_keys
    }

    /// Metadata variables that must exist.
    pub(crate) fn mandatory_keys(&self) -> &HashSet<Key> {
        &self.mandatory_keys
    }

    /// Metadata variables that may exist.
    pub fn metadata_keys(&self) -> &HashSet<Key> {
        &self.metadata_keys
    }

    pub(crate) fn econf_options(&self) -> &EapiEconfOptions {
        &self.econf_options
    }

    fn update_options(mut self, updates: &[(&'static str, bool)]) -> Self {
        for (key, val) in updates.iter() {
            if self.options.insert(key, *val).is_none() {
                panic!("option missing default: {key:?}");
            }
        }
        self
    }

    fn update_phases(mut self, updates: &[(&'static str, PhaseFn)]) -> Self {
        self.phases
            .extend(updates.iter().map(|(s, f)| Phase::new(s, *f)));
        self
    }

    fn update_dep_keys(mut self, updates: &[Key]) -> Self {
        self.dep_keys.extend(updates);
        self.metadata_keys.extend(updates);
        self
    }

    fn update_incremental_keys(mut self, updates: &[Key]) -> Self {
        self.incremental_keys.extend(updates);
        self
    }

    fn update_mandatory_keys(mut self, updates: &[Key]) -> Self {
        self.mandatory_keys.extend(updates);
        self.metadata_keys.extend(updates);
        self
    }

    fn update_metadata_keys(mut self, updates: &[Key]) -> Self {
        self.metadata_keys.extend(updates);
        self
    }

    fn update_econf(mut self, updates: &[EconfUpdate]) -> Self {
        for (opt, markers, val) in updates {
            let markers = markers
                .unwrap_or(&[opt])
                .iter()
                .map(|s| s.to_string())
                .collect();
            let val = val.map(|s| s.to_string());
            self.econf_options.insert(opt, (markers, val));
        }
        self
    }

    fn update_archives(mut self, add: &[&'static str], remove: &[&str]) -> Self {
        self.archives.extend(add);
        for x in remove {
            if !self.archives.remove(*x) {
                panic!("disabling unknown archive format: {x:?}");
            }
        }
        self
    }
}

impl fmt::Display for Eapi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// Get an EAPI given its identifier.
pub fn get_eapi<S: AsRef<str>>(id: S) -> Result<&'static Eapi> {
    let id = id.as_ref();
    match EAPIS.get(id) {
        Some(eapi) => Ok(eapi),
        None => match VALID_EAPI_RE.is_match(id) {
            true => Err(Error::Eapi(format!("unknown EAPI: {id}"))),
            false => Err(Error::Eapi(format!("invalid EAPI: {id}"))),
        },
    }
}

pub static EAPI0: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("0", None)
        .update_phases(&[
            ("pkg_setup", PHASE_STUB),
            ("pkg_config", PHASE_STUB),
            ("pkg_info", PHASE_STUB),
            ("pkg_nofetch", PHASE_STUB),
            ("pkg_prerm", PHASE_STUB),
            ("pkg_postrm", PHASE_STUB),
            ("pkg_preinst", PHASE_STUB),
            ("pkg_postinst", PHASE_STUB),
            ("src_unpack", eapi0::src_unpack),
            ("src_compile", eapi0::src_compile),
            ("src_test", eapi0::src_test),
            ("src_install", PHASE_STUB),
        ])
        .update_dep_keys(&[Depend, Rdepend, Pdepend])
        .update_incremental_keys(&[Iuse, Depend, Rdepend, Pdepend])
        .update_mandatory_keys(&[Description, Slot])
        .update_metadata_keys(&[
            DefinedPhases,
            Eapi,
            Homepage,
            Inherit,
            Inherited,
            Iuse,
            Keywords,
            License,
            Properties,
            Restrict,
            SrcUri,
        ])
        .update_archives(
            &[
                "tar", "gz", "Z", "tar.gz", "tgz", "tar.Z", "bz2", "bz", "tar.bz2", "tbz2",
                "tar.bz", "tbz", "zip", "ZIP", "jar", "7z", "7Z", "rar", "RAR", "LHA", "LHa",
                "lha", "lzh", "a", "deb", "lzma", "tar.lzma",
            ],
            &[],
        )
});

pub static EAPI1: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("1", Some(&EAPI0))
        .update_options(&[("slot_deps", true)])
        .update_phases(&[("src_compile", eapi1::src_compile)])
});

pub static EAPI2: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("2", Some(&EAPI1))
        .update_options(&[
            ("blockers", true),
            ("doman_lang_detect", true),
            ("use_deps", true),
            ("src_uri_renames", true),
        ])
        .update_phases(&[
            ("src_prepare", PHASE_STUB),
            ("src_compile", eapi2::src_compile),
            ("src_configure", eapi2::src_configure),
        ])
});

pub static EAPI3: Lazy<Eapi> =
    Lazy::new(|| Eapi::new("3", Some(&EAPI2)).update_archives(&["tar.xz", "xz"], &[]));

pub static EAPI4: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("4", Some(&EAPI3))
        .update_options(&[
            ("dodoc_recursive", true),
            ("doman_lang_override", true),
            ("rdepend_default", false),
            ("required_use", true),
            ("use_conf_arg", true),
            ("use_dep_defaults", true),
        ])
        .update_phases(&[("pkg_pretend", PHASE_STUB), ("src_install", eapi4::src_install)])
        .update_incremental_keys(&[RequiredUse])
        .update_metadata_keys(&[RequiredUse])
        .update_econf(&[("--disable-dependency-tracking", None, None)])
});

pub static EAPI5: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("5", Some(&EAPI4))
        .update_options(&[
            ("ebuild_phase_func", true),
            ("new_supports_stdin", true),
            ("parallel_tests", true),
            ("required_use_one_of", true),
            ("slot_ops", true),
            ("subslots", true),
        ])
        .update_econf(&[("--disable-silent-rules", None, None)])
});

pub static EAPI6: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("6", Some(&EAPI5))
        .update_options(&[
            ("nonfatal_die", true),
            ("global_failglob", true),
            ("unpack_extended_path", true),
            ("unpack_case_insensitive", true),
        ])
        .update_phases(&[("src_prepare", eapi6::src_prepare), ("src_install", eapi6::src_install)])
        .update_econf(&[
            ("--docdir", None, Some("${EPREFIX}/usr/share/doc/${PF}")),
            ("--htmldir", None, Some("${EPREFIX}/usr/share/doc/${PF}/html")),
        ])
        .update_archives(&["txz"], &[])
});

pub static EAPI7: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("7", Some(&EAPI6))
        .update_options(&[("export_desttree", false), ("export_insdesttree", false)])
        .update_dep_keys(&[Bdepend])
        .update_incremental_keys(&[Bdepend])
        .update_econf(&[("--with-sysroot", None, Some("${ESYSROOT:-/}"))])
});

pub static EAPI8: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("8", Some(&EAPI7))
        .update_options(&[
            ("consistent_file_opts", true),
            ("dosym_relative", true),
            ("src_uri_unrestrict", true),
            ("usev_two_args", true),
        ])
        .update_dep_keys(&[Idepend])
        .update_incremental_keys(&[Idepend, Properties, Restrict])
        .update_econf(&[
            ("--datarootdir", None, Some("${EPREFIX}/usr/share")),
            ("--disable-static", Some(&["--disable-static", "--enable-static"]), None),
        ])
        .update_archives(&[], &["7z", "7Z", "rar", "RAR", "LHA", "LHa", "lha", "lzh"])
});

/// Reference to the latest registered EAPI.
pub static EAPI_LATEST: Lazy<Eapi> = Lazy::new(|| EAPI8.clone());

/// The latest EAPI with extensions on top.
pub static EAPI_PKGCRAFT: Lazy<Eapi> =
    Lazy::new(|| Eapi::new("pkgcraft", Some(&EAPI_LATEST)).update_options(&[("repo_ids", true)]));

/// Ordered mapping of official EAPI identifiers to instances.
pub static EAPIS_OFFICIAL: Lazy<IndexMap<&'static str, &'static Eapi>> = Lazy::new(|| {
    let mut eapis: IndexMap<&'static str, &'static Eapi> = IndexMap::new();
    let mut eapi: &Eapi = &EAPI_LATEST;
    while let Some(x) = eapi.parent {
        eapis.insert(eapi.id, eapi);
        eapi = x;
    }
    eapis.insert(eapi.id, eapi);
    // reverse so it's in chronological order
    eapis.reverse();
    eapis
});

/// Ordered mapping of EAPI identifiers to instances.
pub static EAPIS: Lazy<IndexMap<&'static str, &'static Eapi>> = Lazy::new(|| {
    let mut eapis = EAPIS_OFFICIAL.clone();
    eapis.insert(EAPI_PKGCRAFT.id, &EAPI_PKGCRAFT);
    eapis
});

/// Convert EAPI range into a Vector of EAPI objects, for example "0-" covers all EAPIs and "0~"
/// covers all official EAPIs.
pub(crate) fn supported<S: AsRef<str>>(s: S) -> Result<IndexSet<&'static Eapi>> {
    let (s, max) = match s.as_ref() {
        s if s.ends_with('~') => (s.replace('~', "-"), EAPIS_OFFICIAL.len() - 1),
        s => (s.to_string(), EAPIS.len() - 1),
    };
    let (start, end) = parse::range(&s, max)?;
    Ok((start..=end).map(|n| EAPIS[n]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::assert_err_re;

    #[test]
    fn test_get_eapi() {
        assert!(get_eapi("-invalid").is_err());
        assert!(get_eapi("unknown").is_err());
        assert_eq!(*get_eapi("8").unwrap(), *EAPI8);
    }

    #[test]
    fn test_ordering() {
        assert!(*EAPI0 < *EAPI_LATEST);
        assert!(*EAPI0 <= *EAPI0);
        assert!(*EAPI0 == *EAPI0);
        assert!(*EAPI0 >= *EAPI0);
        assert!(*EAPI_LATEST > *EAPI0);
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
        for (id, eapi) in EAPIS.iter() {
            assert_eq!(format!("{eapi}"), format!("{id}"));
        }
    }

    #[test]
    fn test_atom_parsing() {
        let atom = EAPI0.atom("cat/pkg").unwrap();
        assert_eq!(atom.category(), "cat");
        assert_eq!(atom.package(), "pkg");
        assert_eq!(format!("{atom}"), "cat/pkg");

        let atom = EAPI1.atom("cat/pkg:0").unwrap();
        assert_eq!(atom.category(), "cat");
        assert_eq!(atom.package(), "pkg");
        assert_eq!(atom.slot().unwrap(), "0");
        assert_eq!(format!("{atom}"), "cat/pkg:0");

        let r = EAPI0.atom("cat/pkg:0");
        assert_err_re!(r, format!("invalid atom: \"cat/pkg:0\""));
        let r = EAPI_LATEST.atom("cat/pkg::repo");
        assert_err_re!(r, format!("invalid atom: \"cat/pkg::repo\""));
    }
}
