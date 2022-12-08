use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use camino::Utf8Path;
use indexmap::IndexSet;
use itertools::Either;
use once_cell::sync::{Lazy, OnceCell};
use regex::{escape, Regex, RegexBuilder};
use strum::EnumString;

use crate::archive::Archive;
use crate::atom::Atom;
use crate::metadata::Key::{self, *};
use crate::pkgsh::builtins::{
    BuiltinsMap, Scope, Scopes, ALL, BUILTINS_MAP, GLOBAL, PHASE, PKG, SRC,
};
use crate::pkgsh::phase::Phase::*;
use crate::pkgsh::phase::*;
use crate::pkgsh::BuildVariable::{self, *};
use crate::Error;

static VALID_EAPI_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^[A-Za-z0-9_][A-Za-z0-9+_.-]*$").unwrap());

#[derive(EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum Feature {
    // EAPI 0
    /// RDEPEND=DEPEND if RDEPEND is unset
    RdependDefault,

    // EAPI 1
    /// IUSE defaults
    IuseDefaults,
    /// atom slot deps -- cat/pkg:0
    SlotDeps,

    // EAPI 2
    /// atom blockers -- !cat/pkg and !!cat/pkg
    Blockers,
    /// support language detection via filename for `doman`
    DomanLangDetect,
    /// SRC_URI -> operator for url filename renaming
    SrcUriRenames,
    /// atom use deps -- cat/pkg\[use\]
    UseDeps,

    // EAPI 4
    /// recursive install support via `dodoc -r`
    DodocRecursive,
    /// support `doman` language override via -i18n option
    DomanLangOverride,
    /// atom use defaults -- cat/pkg[use(+)] and cat/pkg[use(-)]
    UseDepDefaults,
    /// REQUIRED_USE support
    RequiredUse,
    /// use_with and use_enable support an optional third argument
    UseConfArg,

    // EAPI 5
    /// new* helpers can use stdin for content instead of a file
    NewSupportsStdin,
    /// running tests in parallel is supported
    ParallelTests,
    /// REQUIRED_USE ?? operator
    RequiredUseOneOf,
    /// atom slot operators -- cat/pkg:=, cat/pkg:*, cat/pkg:0=
    SlotOps,
    /// atom subslots -- cat/pkg:0/4
    Subslots,

    // EAPI 6
    /// `die -n` supports nonfatal usage
    NonfatalDie,
    /// failglob shell option is enabled in global scope
    GlobalFailglob,
    /// `unpack` supports absolute and relative paths
    UnpackExtendedPath,
    /// `unpack` performs case-insensitive file extension matching
    UnpackCaseInsensitive,

    // EAPI 8
    /// improve insopts/exeopts consistency for install functions
    ConsistentFileOpts,
    /// relative path support via `dosym -r`
    DosymRelative,
    /// SRC_URI supports fetch+ and mirror+ prefixes
    SrcUriUnrestrict,
    /// usev supports an optional second arg
    UsevTwoArgs,

    // EAPI EXTENDED
    /// atom repo deps -- cat/pkg::repo
    RepoIds,
}

type EapiEconfOptions = HashMap<String, (IndexSet<String>, Option<String>)>;

/// EAPI object.
#[derive(Default, Clone)]
pub struct Eapi {
    id: String,
    parent: Option<&'static Eapi>,
    features: HashSet<Feature>,
    phases: HashSet<Phase>,
    dep_keys: HashSet<Key>,
    incremental_keys: HashSet<Key>,
    mandatory_keys: HashSet<Key>,
    metadata_keys: HashSet<Key>,
    econf_options: EapiEconfOptions,
    archives: HashSet<String>,
    archives_regex: OnceCell<Regex>,
    env: HashMap<BuildVariable, Scopes>,
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

impl Ord for Eapi {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_index = EAPIS.get_index_of(self.id.as_str()).unwrap();
        let other_index = EAPIS.get_index_of(other.id.as_str()).unwrap();
        self_index.cmp(&other_index)
    }
}

impl PartialOrd for Eapi {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// use the latest EAPI for the Default trait
impl Default for &'static Eapi {
    fn default() -> &'static Eapi {
        &EAPI_PKGCRAFT
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

impl IntoEapi for &str {
    fn into_eapi(self) -> crate::Result<&'static Eapi> {
        <&Eapi>::from_str(self)
    }
}

impl IntoEapi for Option<&str> {
    fn into_eapi(self) -> crate::Result<&'static Eapi> {
        match self {
            None => Ok(Default::default()),
            Some(s) => <&Eapi>::from_str(s),
        }
    }
}

impl IntoEapi for Option<&'static Eapi> {
    fn into_eapi(self) -> crate::Result<&'static Eapi> {
        match self {
            None => Ok(Default::default()),
            Some(eapi) => Ok(eapi),
        }
    }
}

// Used by pkgcraft-c mapping NULL pointers to the default EAPI.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
impl IntoEapi for *const Eapi {
    fn into_eapi(self) -> crate::Result<&'static Eapi> {
        match unsafe { self.as_ref() } {
            Some(p) => Ok(p),
            None => Ok(Default::default()),
        }
    }
}

type EconfUpdate<'a> = (&'a str, Option<&'a [&'a str]>, Option<&'a str>);

impl Eapi {
    fn new(id: &str, parent: Option<&'static Eapi>) -> Eapi {
        let mut eapi = match parent {
            Some(e) => e.clone(),
            None => Eapi::default(),
        };
        eapi.id = id.to_string();
        eapi.parent = parent;
        eapi
    }

    /// Return the EAPI's identifier.
    pub fn as_str(&self) -> &str {
        &self.id
    }

    /// Check if an EAPI has a given feature.
    pub fn has(&self, feature: Feature) -> bool {
        self.features.get(&feature).is_some()
    }

    /// Parse a package atom using EAPI specific support.
    #[inline]
    pub fn atom<S: AsRef<str>>(&'static self, s: S) -> crate::Result<Atom> {
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
                .case_insensitive(self.has(Feature::UnpackCaseInsensitive))
                .build()
                .unwrap()
        })
    }

    pub(crate) fn archive_from_path<P>(&self, path: P) -> crate::Result<(String, Archive)>
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

    pub(crate) fn builtins<S: Into<Scope>>(&self, scope: S) -> &BuiltinsMap {
        let scope = scope.into();
        BUILTINS_MAP
            .get(self)
            .unwrap()
            .get(&scope)
            .unwrap_or_else(|| panic!("EAPI {self}, unknown scope: {scope:?}"))
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

    /// Environment variables that are required to be exported.
    pub(crate) fn env(&self) -> &HashMap<BuildVariable, Scopes> {
        &self.env
    }

    fn enable_features(mut self, features: &[Feature]) -> Self {
        for x in features {
            if !self.features.insert(*x) {
                panic!("EAPI {self}: enabling set feature: {x:?}");
            }
        }
        self
    }

    fn disable_features(mut self, features: &[Feature]) -> Self {
        for x in features {
            if !self.features.remove(x) {
                panic!("EAPI {self}: disabling unset feature: {x:?}");
            }
        }
        self
    }

    fn update_phases(mut self, updates: &[Phase]) -> Self {
        self.phases.extend(updates);
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
            self.econf_options.insert(opt.to_string(), (markers, val));
        }
        self
    }

    fn enable_archives(mut self, types: &[&str]) -> Self {
        self.archives.extend(types.iter().map(|s| s.to_string()));
        self
    }

    fn disable_archives(mut self, types: &[&str]) -> Self {
        for x in types {
            if !self.archives.remove(*x) {
                panic!("disabling unknown archive format: {x:?}");
            }
        }
        self
    }

    fn update_env(mut self, variables: &[(BuildVariable, &[&str])]) -> Self {
        for (var, scopes) in variables {
            self.env.insert(*var, Scopes::new(scopes));
        }
        self
    }

    fn disable_env(mut self, variables: &[BuildVariable]) -> Self {
        for x in variables {
            if self.env.remove(x).is_none() {
                panic!("EAPI {self}: disabling unregistered variable: {x:?}");
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

impl fmt::Debug for Eapi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Eapi {{ id: {} }}", self.id)
    }
}

impl Borrow<str> for &'static Eapi {
    fn borrow(&self) -> &str {
        &self.id
    }
}

impl FromStr for &'static Eapi {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match EAPIS.get(s) {
            Some(eapi) => Ok(eapi),
            None => match VALID_EAPI_RE.is_match(s) {
                true => Err(Error::Eapi(format!("unknown EAPI: {s}"))),
                false => Err(Error::Eapi(format!("invalid EAPI: {s}"))),
            },
        }
    }
}

pub static EAPI0: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("0", None)
        .enable_features(&[Feature::RdependDefault])
        .update_phases(&[
            PkgSetup(PHASE_STUB),
            PkgConfig(PHASE_STUB),
            PkgInfo(PHASE_STUB),
            PkgNofetch(PHASE_STUB),
            PkgPrerm(PHASE_STUB),
            PkgPostrm(PHASE_STUB),
            PkgPreinst(PHASE_STUB),
            PkgPostinst(PHASE_STUB),
            SrcUnpack(PHASE_STUB),
            SrcUnpack(eapi0::src_unpack),
            SrcCompile(eapi0::src_compile),
            SrcTest(eapi0::src_test),
            SrcInstall(PHASE_STUB),
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
        .enable_archives(&[
            "tar", "gz", "Z", "tar.gz", "tgz", "tar.Z", "bz2", "bz", "tar.bz2", "tbz2", "tar.bz",
            "tbz", "zip", "ZIP", "jar", "7z", "7Z", "rar", "RAR", "LHA", "LHa", "lha", "lzh", "a",
            "deb", "lzma", "tar.lzma",
        ])
        .update_env(&[
            (P, &[ALL]),
            (PF, &[ALL]),
            (PN, &[ALL]),
            (CATEGORY, &[ALL]),
            (PV, &[ALL]),
            (PR, &[ALL]),
            (PVR, &[ALL]),
            (A, &[SRC, "pkg_nofetch"]),
            (AA, &[SRC, "pkg_nofetch"]),
            (FILESDIR, &[SRC, GLOBAL]),
            (DISTDIR, &[SRC, GLOBAL]),
            (WORKDIR, &[SRC, GLOBAL]),
            (S, &[SRC]),
            (PORTDIR, &[SRC]),
            (ECLASSDIR, &[SRC]),
            (ROOT, &[PKG]),
            (T, &[ALL]),
            (TMPDIR, &[ALL]),
            (HOME, &[ALL]),
            (D, &["src_install", "pkg_preinst", "pkg_postinst"]),
            (DESTTREE, &["src_install"]),
            (INSDESTTREE, &["src_install"]),
            (USE, &[ALL]),
            (EBUILD_PHASE, &[PHASE]),
            (KV, &[ALL]),
        ])
});

pub static EAPI1: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("1", Some(&EAPI0))
        .enable_features(&[Feature::IuseDefaults, Feature::SlotDeps])
        .update_phases(&[SrcCompile(eapi1::src_compile)])
});

pub static EAPI2: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("2", Some(&EAPI1))
        .enable_features(&[
            Feature::Blockers,
            Feature::DomanLangDetect,
            Feature::UseDeps,
            Feature::SrcUriRenames,
        ])
        .update_phases(&[
            SrcPrepare(PHASE_STUB),
            SrcCompile(eapi2::src_compile),
            SrcConfigure(eapi2::src_configure),
        ])
});

pub static EAPI3: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("3", Some(&EAPI2))
        .enable_archives(&["tar.xz", "xz"])
        .update_env(&[
            (EPREFIX, &[GLOBAL]),
            (ED, &["src_install", "pkg_preinst", "pkg_postinst"]),
            (EROOT, &[PKG]),
        ])
});

pub static EAPI4: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("4", Some(&EAPI3))
        .enable_features(&[
            Feature::DodocRecursive,
            Feature::DomanLangOverride,
            Feature::RequiredUse,
            Feature::UseConfArg,
            Feature::UseDepDefaults,
        ])
        .disable_features(&[Feature::RdependDefault])
        .update_phases(&[PkgPretend(PHASE_STUB), SrcInstall(eapi4::src_install)])
        .update_incremental_keys(&[RequiredUse])
        .update_metadata_keys(&[RequiredUse])
        .update_econf(&[("--disable-dependency-tracking", None, None)])
        .update_env(&[
            (MERGE_TYPE, &[PKG]),
            (REPLACING_VERSIONS, &[PKG]),
            (REPLACED_BY_VERSION, &["pkg_prerm", "pkg_postrm"]),
        ])
        .disable_env(&[AA, KV])
});

pub static EAPI5: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("5", Some(&EAPI4))
        .enable_features(&[
            Feature::NewSupportsStdin,
            Feature::ParallelTests,
            Feature::RequiredUseOneOf,
            Feature::SlotOps,
            Feature::Subslots,
        ])
        .update_econf(&[("--disable-silent-rules", None, None)])
        .update_env(&[(EBUILD_PHASE_FUNC, &[PHASE])])
});

pub static EAPI6: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("6", Some(&EAPI5))
        .enable_features(&[
            Feature::NonfatalDie,
            Feature::GlobalFailglob,
            Feature::UnpackExtendedPath,
            Feature::UnpackCaseInsensitive,
        ])
        .update_phases(&[SrcPrepare(eapi6::src_prepare), SrcInstall(eapi6::src_install)])
        .update_econf(&[
            ("--docdir", None, Some("${EPREFIX}/usr/share/doc/${PF}")),
            ("--htmldir", None, Some("${EPREFIX}/usr/share/doc/${PF}/html")),
        ])
        .enable_archives(&["txz"])
});

pub static EAPI7: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("7", Some(&EAPI6))
        .update_dep_keys(&[Bdepend])
        .update_incremental_keys(&[Bdepend])
        .update_econf(&[("--with-sysroot", None, Some("${ESYSROOT:-/}"))])
        .update_env(&[
            (SYSROOT, &[SRC, "pkg_setup"]),
            (ESYSROOT, &[SRC, "pkg_setup"]),
            (BROOT, &[SRC, "pkg_setup"]),
        ])
        .disable_env(&[PORTDIR, ECLASSDIR, DESTTREE, INSDESTTREE])
});

pub static EAPI8: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("8", Some(&EAPI7))
        .enable_features(&[
            Feature::ConsistentFileOpts,
            Feature::DosymRelative,
            Feature::SrcUriUnrestrict,
            Feature::UsevTwoArgs,
        ])
        .update_dep_keys(&[Idepend])
        .update_incremental_keys(&[Idepend, Properties, Restrict])
        .update_econf(&[
            ("--datarootdir", None, Some("${EPREFIX}/usr/share")),
            ("--disable-static", Some(&["--disable-static", "--enable-static"]), None),
        ])
        .disable_archives(&["7z", "7Z", "rar", "RAR", "LHA", "LHa", "lha", "lzh"])
});

/// Reference to the latest registered EAPI.
pub static EAPI_LATEST: Lazy<Eapi> = Lazy::new(|| EAPI8.clone());

/// The latest EAPI with extensions on top.
pub static EAPI_PKGCRAFT: Lazy<Eapi> =
    Lazy::new(|| Eapi::new("pkgcraft", Some(&EAPI_LATEST)).enable_features(&[Feature::RepoIds]));

/// Ordered mapping of official EAPI identifiers to instances.
pub static EAPIS_OFFICIAL: Lazy<IndexSet<&'static Eapi>> = Lazy::new(|| {
    let mut eapis = IndexSet::new();
    let mut eapi: &Eapi = &EAPI_LATEST;
    while let Some(x) = eapi.parent {
        eapis.insert(eapi);
        eapi = x;
    }
    eapis.insert(eapi);
    // reverse so it's in chronological order
    eapis.reverse();
    eapis
});

/// Ordered mapping of unofficial EAPI identifiers to instances.
pub static EAPIS_UNOFFICIAL: Lazy<IndexSet<&'static Eapi>> =
    Lazy::new(|| [&*EAPI_PKGCRAFT].into_iter().collect());

/// Ordered mapping of EAPI identifiers to instances.
pub static EAPIS: Lazy<IndexSet<&'static Eapi>> = Lazy::new(|| {
    EAPIS_OFFICIAL
        .iter()
        .copied()
        .chain(EAPIS_UNOFFICIAL.iter().copied())
        .collect()
});

/// Convert EAPI range into an ordered set of EAPI objects.
pub fn range(s: &str) -> crate::Result<impl Iterator<Item = &'static Eapi>> {
    let err = || Error::InvalidValue(format!("invalid EAPI range: {s}"));

    // convert EAPI identifier to index, "U" being an alias for the first unofficial EAPI
    let eapi_idx = |s: &str| match s {
        "U" => Ok(EAPIS.get_index_of(EAPIS_UNOFFICIAL[0].as_str()).unwrap()),
        _ => EAPIS.get_index_of(s).ok_or_else(err),
    };

    // determine range operator
    let mut inclusive = true;
    let (start, end) = s
        .split_once("..=")
        .or_else(|| {
            inclusive = false;
            s.split_once("..")
        })
        .ok_or_else(err)?;

    // convert strings into Option<s> if non-empty, otherwise None
    let start = (!start.is_empty()).then_some(start);
    let end = (!end.is_empty()).then_some(end);

    // determine the range start and end points
    let (start, end) = match (start, end) {
        (None, None) if !inclusive => (0, EAPIS.len()),
        (None, Some(e)) => (0, eapi_idx(e)?),
        (Some(s), None) if !inclusive => (eapi_idx(s)?, EAPIS.len()),
        (Some(s), Some(e)) => (eapi_idx(s)?, eapi_idx(e)?),
        _ => return Err(err()),
    };

    let eapis = match inclusive {
        false => Either::Left((start..end).map(|n| EAPIS[n])),
        true => Either::Right((start..=end).map(|n| EAPIS[n])),
    };

    Ok(eapis)
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use crate::macros::assert_err_re;
    use crate::test::assert_ordered_eq;

    use super::*;

    #[test]
    fn test_from_str() {
        assert!(<&Eapi>::from_str("-invalid").is_err());
        assert!(<&Eapi>::from_str("unknown").is_err());
        assert_eq!(<&Eapi>::from_str("8").unwrap(), &*EAPI8);
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
        assert!(!EAPI0.has(Feature::UseDeps));
        assert!(EAPI_LATEST.has(Feature::UseDeps));
    }

    #[test]
    fn test_atom_parsing() {
        let atom = EAPI0.atom("cat/pkg").unwrap();
        assert_eq!(atom.category(), "cat");
        assert_eq!(atom.package(), "pkg");
        assert_eq!(atom.to_string(), "cat/pkg");

        let atom = EAPI1.atom("cat/pkg:0").unwrap();
        assert_eq!(atom.category(), "cat");
        assert_eq!(atom.package(), "pkg");
        assert_eq!(atom.slot().unwrap(), "0");
        assert_eq!(atom.to_string(), "cat/pkg:0");

        let r = EAPI0.atom("cat/pkg:0");
        assert_err_re!(r, "invalid atom: cat/pkg:0");
        let r = EAPI_LATEST.atom("cat/pkg::repo");
        assert_err_re!(r, "invalid atom: cat/pkg::repo");
    }

    #[test]
    fn test_into_eapi() {
        assert_eq!(&*EAPI0, EAPI0.into_eapi().unwrap());
        assert_eq!(&*EAPI1, "1".into_eapi().unwrap());

        let mut arg: Option<&str> = None;
        assert_eq!(&*EAPI_PKGCRAFT, arg.into_eapi().unwrap());
        arg = Some("1");
        assert_eq!(&*EAPI1, arg.into_eapi().unwrap());

        let mut arg: Option<&'static Eapi> = None;
        assert_eq!(&*EAPI_PKGCRAFT, arg.into_eapi().unwrap());
        arg = Some(&EAPI1);
        assert_eq!(&*EAPI1, arg.into_eapi().unwrap());

        let mut arg: *const Eapi = ptr::null();
        assert_eq!(&*EAPI_PKGCRAFT, arg.into_eapi().unwrap());
        arg = &*EAPI1 as *const _;
        assert_eq!(&*EAPI1, arg.into_eapi().unwrap());
    }

    #[test]
    fn test_builtins() {
        let static_scopes: Vec<Scope> = vec![Scope::Global, Scope::Eclass];
        for eapi in EAPIS.iter() {
            let phase_scopes: Vec<Scope> = eapi.phases().iter().map(|p| p.into()).collect();
            let scopes = static_scopes.iter().chain(phase_scopes.iter());
            for scope in scopes {
                assert!(
                    !eapi.builtins(*scope).is_empty(),
                    "EAPI {eapi} failed for scope: {scope:?}"
                );
            }
        }
    }

    #[test]
    fn test_range() {
        // invalid
        for s in ["", "1", "1..=", "..=", "...", "0-", "-1..", "1..9999", "..=unknown"] {
            let r = range(s);
            assert!(r.is_err(), "range didn't fail: {s}");
        }

        assert_ordered_eq(range("..").unwrap(), EAPIS.iter().copied());
        assert_ordered_eq(range("..U").unwrap(), EAPIS_OFFICIAL.iter().copied());
        assert_ordered_eq(range("U..").unwrap(), EAPIS_UNOFFICIAL.iter().copied());
        assert!(range("1..1").unwrap().next().is_none());
        assert_ordered_eq(range("1..2").unwrap(), [&*EAPI1]);
        assert_ordered_eq(range("1..=2").unwrap(), [&*EAPI1, &*EAPI2]);
        assert_ordered_eq(range("..=2").unwrap(), [&*EAPI0, &*EAPI1, &*EAPI2]);
    }
}
