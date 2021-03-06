use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::os::raw::c_char;

use camino::Utf8Path;
use indexmap::{IndexMap, IndexSet};
use once_cell::sync::{Lazy, OnceCell};
use regex::{escape, Regex, RegexBuilder};
use scallop::functions;
use scallop::variables::string_value;
use strum::{AsRefStr, Display, EnumString};

use crate::archive::Archive;
use crate::atom::Atom;
use crate::pkgsh::builtins::{parse, BuiltinsMap, Scope, BUILTINS_MAP};
use crate::pkgsh::phase::Phase::*;
use crate::pkgsh::phase::*;
use crate::Error;

static VALID_EAPI_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^[A-Za-z0-9_][A-Za-z0-9+_.-]*$").unwrap());

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub(crate) enum Feature {
    // EAPI 0
    /// RDEPEND=DEPEND if RDEPEND is unset
    RdependDefault,
    /// DESTTREE is exported to the ebuild env
    ExportDesttree,
    /// INSDESTTREE is exported to the ebuild env
    ExportInsdesttree,

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
    /// export the running phase name as $EBUILD_PHASE_FUNC
    EbuildPhaseFunc,
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
        let self_index = EAPIS.get_index_of(self.id.as_str()).unwrap();
        let other_index = EAPIS.get_index_of(other.id.as_str()).unwrap();
        self_index.partial_cmp(&other_index)
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
        get_eapi(self)
    }
}

impl IntoEapi for Option<&str> {
    fn into_eapi(self) -> crate::Result<&'static Eapi> {
        match self {
            None => Ok(Default::default()),
            Some(s) => get_eapi(s),
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
impl IntoEapi for *const c_char {
    fn into_eapi(self) -> crate::Result<&'static Eapi> {
        match self.is_null() {
            true => Ok(Default::default()),
            false => get_eapi(unsafe { CStr::from_ptr(self).to_string_lossy() }),
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
    pub(crate) fn has(&self, feature: Feature) -> bool {
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

/// Get an EAPI given its identifier.
pub fn get_eapi<S: AsRef<str>>(id: S) -> crate::Result<&'static Eapi> {
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
        .enable_features(&[
            Feature::RdependDefault,
            Feature::ExportDesttree,
            Feature::ExportInsdesttree,
        ])
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

pub static EAPI3: Lazy<Eapi> =
    Lazy::new(|| Eapi::new("3", Some(&EAPI2)).enable_archives(&["tar.xz", "xz"]));

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
});

pub static EAPI5: Lazy<Eapi> = Lazy::new(|| {
    Eapi::new("5", Some(&EAPI4))
        .enable_features(&[
            Feature::EbuildPhaseFunc,
            Feature::NewSupportsStdin,
            Feature::ParallelTests,
            Feature::RequiredUseOneOf,
            Feature::SlotOps,
            Feature::Subslots,
        ])
        .update_econf(&[("--disable-silent-rules", None, None)])
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
        .disable_features(&[Feature::ExportDesttree, Feature::ExportInsdesttree])
        .update_dep_keys(&[Bdepend])
        .update_incremental_keys(&[Bdepend])
        .update_econf(&[("--with-sysroot", None, Some("${ESYSROOT:-/}"))])
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
pub static EAPIS_OFFICIAL: Lazy<IndexMap<String, &'static Eapi>> = Lazy::new(|| {
    let mut eapis: IndexMap<String, &'static Eapi> = IndexMap::new();
    let mut eapi: &Eapi = &EAPI_LATEST;
    while let Some(x) = eapi.parent {
        eapis.insert(eapi.id.clone(), eapi);
        eapi = x;
    }
    eapis.insert(eapi.id.clone(), eapi);
    // reverse so it's in chronological order
    eapis.reverse();
    eapis
});

/// Ordered mapping of EAPI identifiers to instances.
pub static EAPIS: Lazy<IndexMap<String, &'static Eapi>> = Lazy::new(|| {
    let mut eapis = EAPIS_OFFICIAL.clone();
    eapis.insert(EAPI_PKGCRAFT.id.clone(), &EAPI_PKGCRAFT);
    eapis
});

/// Convert EAPI range into a Vector of EAPI objects, for example "0-" covers all EAPIs and "0~"
/// covers all official EAPIs.
pub(crate) fn supported<S: AsRef<str>>(s: S) -> crate::Result<IndexSet<&'static Eapi>> {
    let (s, max) = match s.as_ref() {
        s if s.ends_with('~') => (s.replace('~', "-"), EAPIS_OFFICIAL.len() - 1),
        s => (s.to_string(), EAPIS.len() - 1),
    };
    let (start, end) = parse::range(&s, max)?;
    Ok((start..=end).map(|n| EAPIS[n]).collect())
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::ptr;

    use crate::macros::assert_err_re;

    use super::*;

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
        assert!(!EAPI0.has(Feature::UseDeps));
        assert!(EAPI_LATEST.has(Feature::UseDeps));
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

        let mut arg: *const c_char = ptr::null();
        assert_eq!(&*EAPI_PKGCRAFT, arg.into_eapi().unwrap());
        let s = CString::new("1").unwrap();
        arg = s.as_ptr();
        assert_eq!(&*EAPI1, arg.into_eapi().unwrap());
    }

    #[test]
    fn test_builtins() {
        let static_scopes: Vec<Scope> = vec![Scope::Global, Scope::Eclass];
        for eapi in EAPIS.values() {
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
}
