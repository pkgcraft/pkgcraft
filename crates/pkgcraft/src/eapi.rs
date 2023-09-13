use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::{fmt, fs, io};

use camino::Utf8Path;
use indexmap::IndexSet;
use itertools::Either;
use once_cell::sync::Lazy;
use strum::EnumString;

use crate::archive::Archive;
use crate::dep::Dep;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::Restriction;
use crate::shell::hooks::{Hook, HookKind};
use crate::shell::metadata::Key;
use crate::shell::operations::{Operation, OperationKind};
use crate::shell::phase::{Phase, PhaseKind};
use crate::shell::scope::{Scope, Scopes};
use crate::shell::BuildVariable;
use crate::Error;

peg::parser!(grammar parse() for str {
    // EAPIs must not begin with a hyphen, dot, or plus sign.
    pub(super) rule eapi_str() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
        } / expected!("EAPI"))
        { s }

    rule optionally_quoted<T>(expr: rule<T>) -> T
        = s:expr() { s }
        / "\"" s:expr() "\"" { s }
        / "\'" s:expr() "\'" { s }

    pub(super) rule eapi_value() -> &'input str
        = s:optionally_quoted(<eapi_str()>) { s }
});

pub(crate) fn parse_value(s: &str) -> crate::Result<&str> {
    parse::eapi_value(s).map_err(|_| Error::InvalidValue(format!("invalid EAPI: {s}")))
}

/// Features that relate to differentiation between EAPIs as specified by PMS.
#[derive(EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum Feature {
    // EAPI 0
    /// RDEPEND=DEPEND if RDEPEND is unset
    RdependDefault,

    // EAPI 1
    /// IUSE defaults
    IuseDefaults,
    /// slot deps -- cat/pkg:0
    SlotDeps,

    // EAPI 2
    /// blockers -- !cat/pkg and !!cat/pkg
    Blockers,
    /// support language detection via filename for `doman`
    DomanLangDetect,
    /// SRC_URI -> operator for url filename renaming
    SrcUriRenames,
    /// use deps -- cat/pkg\[use\]
    UseDeps,

    // EAPI 4
    /// recursive install support via `dodoc -r`
    DodocRecursive,
    /// support `doman` language override via -i18n option
    DomanLangOverride,
    /// use defaults -- cat/pkg[use(+)] and cat/pkg[use(-)]
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
    /// slot operators -- cat/pkg:=, cat/pkg:*, cat/pkg:0=
    SlotOps,
    /// subslots -- cat/pkg:0/4
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

    // EAPI 7
    /// path variables ROOT, EROOT, D, and ED end with a trailing slash
    TrailingSlash,

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
    /// repo deps -- cat/pkg::repo
    RepoIds,
}

type EconfUpdate<'a> = (&'a str, Option<&'a [&'a str]>, Option<&'a str>);
type EapiEconfOptions = HashMap<String, (IndexSet<String>, Option<String>)>;

/// EAPI object.
#[derive(Default, Clone)]
pub struct Eapi {
    id: String,
    parent: Option<&'static Self>,
    features: IndexSet<Feature>,
    operations: IndexSet<Operation>,
    phases: IndexSet<Phase>,
    dep_keys: IndexSet<Key>,
    incremental_keys: IndexSet<Key>,
    mandatory_keys: IndexSet<Key>,
    metadata_keys: IndexSet<Key>,
    econf_options: EapiEconfOptions,
    archives: IndexSet<String>,
    env: HashMap<BuildVariable, HashSet<Scope>>,
    hooks: HashMap<PhaseKind, HashMap<HookKind, IndexSet<Hook>>>,
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

impl TryFrom<&str> for &'static Eapi {
    type Error = Error;

    fn try_from(value: &str) -> crate::Result<&'static Eapi> {
        <&Eapi>::from_str(value)
    }
}

impl TryFrom<Option<&str>> for &'static Eapi {
    type Error = Error;

    fn try_from(value: Option<&str>) -> crate::Result<&'static Eapi> {
        value.map_or(Ok(Default::default()), <&Eapi>::from_str)
    }
}

impl TryFrom<Option<&'static Eapi>> for &'static Eapi {
    type Error = Error;

    fn try_from(value: Option<&'static Eapi>) -> crate::Result<&'static Eapi> {
        Ok(value.unwrap_or_default())
    }
}

impl TryFrom<&Utf8Path> for &'static Eapi {
    type Error = Error;

    fn try_from(value: &Utf8Path) -> crate::Result<&'static Eapi> {
        match fs::read_to_string(value) {
            Ok(s) => <&Eapi>::from_str(s.trim_end()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(&*EAPI0),
            Err(e) => Err(Error::IO(format!("failed reading EAPI: {value}: {e}"))),
        }
    }
}

impl Eapi {
    /// Create a new Eapi given an identifier and optional parent to inherit from.
    fn new(id: &str, parent: Option<&'static Eapi>) -> Self {
        let mut eapi = parent.cloned().unwrap_or_default();
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

    /// Parse a package dependency using a specific EAPI.
    pub fn dep<S: AsRef<str>>(&'static self, s: S) -> crate::Result<Dep> {
        Dep::new(s.as_ref(), self)
    }

    /// Return the ordered set of phases for a given operation.
    pub(crate) fn operation(&self, op: OperationKind) -> crate::Result<&Operation> {
        self.operations
            .get(&op)
            .ok_or_else(|| Error::InvalidValue(format!("EAPI {self}: unknown operation: {op}")))
    }

    /// Return all the known phases for an EAPI.
    pub(crate) fn phases(&self) -> &IndexSet<Phase> {
        &self.phases
    }

    /// Load an archive from a given path if it's supported.
    pub(crate) fn archive_from_path<P>(&self, path: P) -> crate::Result<(&str, Archive)>
    where
        P: AsRef<Utf8Path>,
    {
        let path = path.as_ref();

        let matches = |path: &Utf8Path, ext: &str| -> bool {
            if self.has(Feature::UnpackCaseInsensitive) {
                let ext = format!(".{}", ext.to_lowercase());
                path.as_str().to_lowercase().ends_with(&ext)
            } else {
                let ext = format!(".{ext}");
                path.as_str().ends_with(&ext)
            }
        };

        for ext in &self.archives {
            if matches(path, ext) {
                let archive = Archive::from_path(path)?;
                return Ok((ext, archive));
            }
        }

        Err(Error::InvalidValue(format!("unknown archive format: {path}")))
    }

    /// Metadata variables for dependencies.
    pub fn dep_keys(&self) -> &IndexSet<Key> {
        &self.dep_keys
    }

    /// Metadata variables that are incrementally handled.
    pub(crate) fn incremental_keys(&self) -> &IndexSet<Key> {
        &self.incremental_keys
    }

    /// Metadata variables that must exist.
    pub(crate) fn mandatory_keys(&self) -> &IndexSet<Key> {
        &self.mandatory_keys
    }

    /// Metadata variables that may exist.
    pub fn metadata_keys(&self) -> &IndexSet<Key> {
        &self.metadata_keys
    }

    /// Return all EAPI-specific econf options.
    pub(crate) fn econf_options(&self) -> &EapiEconfOptions {
        &self.econf_options
    }

    /// Return the mapping of all exported environment variables.
    pub(crate) fn env(&self) -> &HashMap<BuildVariable, HashSet<Scope>> {
        &self.env
    }

    /// Return the hooks for a given Phase.
    pub(crate) fn hooks(&self) -> &HashMap<PhaseKind, HashMap<HookKind, IndexSet<Hook>>> {
        &self.hooks
    }

    /// Enable features during Eapi registration.
    fn enable_features(mut self, features: &[Feature]) -> Self {
        for x in features {
            if !self.features.insert(*x) {
                panic!("EAPI {self}: enabling set feature: {x:?}");
            }
        }
        self.features.sort();
        self
    }

    /// Disable inherited features during Eapi registration.
    fn disable_features(mut self, features: &[Feature]) -> Self {
        for x in features {
            if !self.features.remove(x) {
                panic!("EAPI {self}: disabling unset feature: {x:?}");
            }
        }
        self.features.sort();
        self
    }

    /// Update operations during Eapi registration.
    fn update_operations<I>(mut self, operations: I) -> Self
    where
        I: IntoIterator<Item = Operation>,
    {
        for op in operations {
            self.operations.replace(op);
        }
        self.operations.sort();
        self
    }

    /// Update phases for all known operations during Eapi registration.
    fn update_phases<I>(mut self, phases: I) -> Self
    where
        I: IntoIterator<Item = Phase>,
    {
        let phases: Vec<_> = phases.into_iter().collect();

        // replace phases registered into operations with new phases
        self.operations = self
            .operations
            .into_iter()
            .map(|mut op| {
                for phase in &phases {
                    op.phases.replace(*phase);
                }
                op
            })
            .collect();
        self
    }

    /// Update dependency types during Eapi registration.
    fn update_dep_keys(mut self, updates: &[Key]) -> Self {
        self.dep_keys.extend(updates);
        self.dep_keys.sort();
        self.metadata_keys.extend(updates);
        self.metadata_keys.sort();
        self
    }

    /// Update incremental variables during Eapi registration.
    fn update_incremental_keys(mut self, updates: &[Key]) -> Self {
        self.incremental_keys.extend(updates);
        self.incremental_keys.sort();
        self
    }

    /// Update mandatory metadata variables during Eapi registration.
    fn update_mandatory_keys(mut self, updates: &[Key]) -> Self {
        self.mandatory_keys.extend(updates);
        self.mandatory_keys.sort();
        self.metadata_keys.extend(updates);
        self.metadata_keys.sort();
        self
    }

    /// Update metadata variables during Eapi registration.
    fn update_metadata_keys(mut self, updates: &[Key]) -> Self {
        self.metadata_keys.extend(updates);
        self.metadata_keys.sort();
        self
    }

    /// Update econf options during Eapi registration.
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

    /// Enable support for archive extensions during Eapi registration.
    fn enable_archives(mut self, types: &[&str]) -> Self {
        self.archives.extend(types.iter().map(|s| s.to_string()));
        // sort archives by extension length, longest to shortest.
        self.archives.sort_by(|s1, s2| s1.len().cmp(&s2.len()));
        self.archives.reverse();
        self
    }

    /// Disable support for archive extensions during Eapi registration.
    fn disable_archives(mut self, types: &[&str]) -> Self {
        for x in types {
            if !self.archives.remove(*x) {
                panic!("disabling unknown archive format: {x:?}");
            }
        }
        self
    }

    /// Enable support for build variables during Eapi registration.
    fn update_env(mut self, variables: &[(BuildVariable, &[Scopes])]) -> Self {
        for (var, scopes) in variables.iter() {
            let scopes: HashSet<_> = scopes.iter().flatten().collect();
            self.env.insert(*var, scopes);
        }
        self
    }

    /// Disable support for build variables during Eapi registration.
    fn disable_env(mut self, variables: &[BuildVariable]) -> Self {
        for x in variables {
            if self.env.remove(x).is_none() {
                panic!("EAPI {self}: disabling unregistered variable: {x:?}");
            }
        }
        self
    }

    /// Update incremental variables during Eapi registration.
    fn update_hooks(mut self, values: &[Hook]) -> Self {
        for hook in values {
            let hooks = self
                .hooks
                .entry(hook.phase)
                .or_insert_with(HashMap::new)
                .entry(hook.kind)
                .or_insert_with(IndexSet::new);
            hooks.insert(hook.clone());
            hooks.sort();
        }
        self
    }

    /// Finalize remaining fields that depend on previous fields.
    fn finalize(mut self) -> Self {
        self.phases = self.operations.iter().flatten().copied().collect();
        self.phases.sort();
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
        if let Some(eapi) = EAPIS.get(s) {
            Ok(eapi)
        } else if parse::eapi_str(s).is_ok() {
            Err(Error::InvalidValue(format!("unknown EAPI: {s}")))
        } else {
            Err(Error::InvalidValue(format!("invalid EAPI: {s}")))
        }
    }
}

pub static EAPI0: Lazy<Eapi> = Lazy::new(|| {
    use crate::shell::operations::OperationKind::*;
    use crate::shell::phase::{PhaseKind::*, *};
    use crate::shell::scope::Scopes::*;
    use crate::shell::BuildVariable::*;
    use Feature::*;

    Eapi::new("0", None)
        .enable_features(&[RdependDefault, TrailingSlash])
        .update_operations([
            Build.op([
                PkgSetup.func(None),
                SrcUnpack.func(Some(eapi0::src_unpack)),
                SrcCompile.func(Some(eapi0::src_compile)),
                SrcTest.func(Some(eapi0::src_test)),
                SrcInstall.func(None),
            ]),
            Install.op([PkgPreinst.func(None), PkgPostinst.func(None)]),
            Uninstall.op([PkgPrerm.func(None), PkgPostrm.func(None)]),
            Replace.op([
                PkgPreinst.func(None),
                PkgPrerm.func(None),
                PkgPostrm.func(None),
                PkgPostinst.func(None),
            ]),
            Config.op([PkgConfig.func(None)]),
            Info.op([PkgInfo.func(None)]),
            NoFetch.op([PkgNofetch.func(Some(eapi0::pkg_nofetch))]),
        ])
        .update_dep_keys(&[Key::Depend, Key::Rdepend, Key::Pdepend])
        .update_incremental_keys(&[Key::Iuse, Key::Depend, Key::Rdepend, Key::Pdepend])
        .update_mandatory_keys(&[Key::Description, Key::Slot])
        .update_metadata_keys(&[
            Key::DefinedPhases,
            Key::Eapi,
            Key::Homepage,
            Key::Inherit,
            Key::Inherited,
            Key::Iuse,
            Key::Keywords,
            Key::License,
            Key::Properties,
            Key::Restrict,
            Key::SrcUri,
        ])
        .enable_archives(&[
            "tar", "gz", "Z", "tar.gz", "tgz", "tar.Z", "bz2", "bz", "tar.bz2", "tbz2", "tar.bz",
            "tbz", "zip", "ZIP", "jar", "7z", "7Z", "rar", "RAR", "LHA", "LHa", "lha", "lzh", "a",
            "deb", "lzma", "tar.lzma",
        ])
        .update_env(&[
            (P, &[All]),
            (PF, &[All]),
            (PN, &[All]),
            (CATEGORY, &[All]),
            (PV, &[All]),
            (PR, &[All]),
            (PVR, &[All]),
            (A, &[Src, Phase(PkgNofetch)]),
            (AA, &[Src, Phase(PkgNofetch)]),
            (FILESDIR, &[Src, Global]),
            (DISTDIR, &[Src, Global]),
            (WORKDIR, &[Src, Global]),
            (S, &[Src]),
            (PORTDIR, &[Src]),
            (ECLASSDIR, &[Src]),
            (ROOT, &[Pkg]),
            (T, &[All]),
            (TMPDIR, &[All]),
            (HOME, &[All]),
            (D, &[Phase(SrcInstall), Phase(PkgPreinst), Phase(PkgPostinst)]),
            (DESTTREE, &[Phase(SrcInstall)]),
            (INSDESTTREE, &[Phase(SrcInstall)]),
            (USE, &[All]),
            (EBUILD_PHASE, &[Phases]),
            (KV, &[All]),
        ])
        .finalize()
});

pub static EAPI1: Lazy<Eapi> = Lazy::new(|| {
    use crate::shell::phase::{PhaseKind::*, *};
    use Feature::*;

    Eapi::new("1", Some(&EAPI0))
        .enable_features(&[IuseDefaults, SlotDeps])
        .update_phases([SrcCompile.func(Some(eapi1::src_compile))])
        .finalize()
});

pub static EAPI2: Lazy<Eapi> = Lazy::new(|| {
    use crate::shell::operations::OperationKind::*;
    use crate::shell::phase::{PhaseKind::*, *};
    use Feature::*;

    Eapi::new("2", Some(&EAPI1))
        .enable_features(&[Blockers, DomanLangDetect, UseDeps, SrcUriRenames])
        .update_operations([Build.op([
            PkgSetup.func(None),
            SrcUnpack.func(Some(eapi0::src_unpack)),
            SrcPrepare.func(None),
            SrcConfigure.func(Some(eapi2::src_configure)),
            SrcCompile.func(Some(eapi2::src_compile)),
            SrcTest.func(Some(eapi0::src_test)),
            SrcInstall.func(None),
        ])])
        .finalize()
});

pub static EAPI3: Lazy<Eapi> = Lazy::new(|| {
    use crate::shell::phase::PhaseKind::*;
    use crate::shell::scope::Scopes::*;
    use crate::shell::BuildVariable::*;

    Eapi::new("3", Some(&EAPI2))
        .enable_archives(&["tar.xz", "xz"])
        .update_env(&[
            (EPREFIX, &[Global]),
            (ED, &[Phase(SrcInstall), Phase(PkgPreinst), Phase(PkgPostinst)]),
            (EROOT, &[Pkg]),
        ])
        .finalize()
});

pub static EAPI4: Lazy<Eapi> = Lazy::new(|| {
    use crate::shell::hooks::eapi4::HOOKS;
    use crate::shell::operations::OperationKind::*;
    use crate::shell::phase::{PhaseKind::*, *};
    use crate::shell::scope::Scopes::*;
    use crate::shell::BuildVariable::*;
    use Feature::*;

    Eapi::new("4", Some(&EAPI3))
        .enable_features(&[
            DodocRecursive,
            DomanLangOverride,
            RequiredUse,
            UseConfArg,
            UseDepDefaults,
        ])
        .disable_features(&[RdependDefault])
        .update_operations([Pretend.op([PkgPretend.func(None)])])
        .update_phases([SrcInstall.func(Some(eapi4::src_install))])
        .update_incremental_keys(&[Key::RequiredUse])
        .update_metadata_keys(&[Key::RequiredUse])
        .update_econf(&[("--disable-dependency-tracking", None, None)])
        .update_env(&[
            (MERGE_TYPE, &[Pkg]),
            (REPLACING_VERSIONS, &[Pkg]),
            (REPLACED_BY_VERSION, &[Phase(PkgPrerm), Phase(PkgPostrm)]),
        ])
        .disable_env(&[AA, KV])
        .update_hooks(&HOOKS)
        .finalize()
});

pub static EAPI5: Lazy<Eapi> = Lazy::new(|| {
    use crate::shell::scope::Scopes::*;
    use crate::shell::BuildVariable::*;
    use Feature::*;

    Eapi::new("5", Some(&EAPI4))
        .enable_features(&[NewSupportsStdin, ParallelTests, RequiredUseOneOf, SlotOps, Subslots])
        .update_econf(&[("--disable-silent-rules", None, None)])
        .update_env(&[(EBUILD_PHASE_FUNC, &[Phases])])
        .finalize()
});

pub static EAPI6: Lazy<Eapi> = Lazy::new(|| {
    use crate::shell::phase::{PhaseKind::*, *};
    use Feature::*;

    Eapi::new("6", Some(&EAPI5))
        .enable_features(&[NonfatalDie, GlobalFailglob, UnpackExtendedPath, UnpackCaseInsensitive])
        .update_phases([
            SrcPrepare.func(Some(eapi6::src_prepare)),
            SrcInstall.func(Some(eapi6::src_install)),
        ])
        .update_econf(&[
            ("--docdir", None, Some("${EPREFIX}/usr/share/doc/${PF}")),
            ("--htmldir", None, Some("${EPREFIX}/usr/share/doc/${PF}/html")),
        ])
        .enable_archives(&["txz"])
        .finalize()
});

pub static EAPI7: Lazy<Eapi> = Lazy::new(|| {
    use crate::shell::hooks::eapi7::HOOKS;
    use crate::shell::phase::PhaseKind::*;
    use crate::shell::scope::Scopes::*;
    use crate::shell::BuildVariable::*;
    use Feature::*;

    Eapi::new("7", Some(&EAPI6))
        .disable_features(&[TrailingSlash])
        .update_dep_keys(&[Key::Bdepend])
        .update_incremental_keys(&[Key::Bdepend])
        .update_econf(&[("--with-sysroot", None, Some("${ESYSROOT:-/}"))])
        .update_env(&[
            (SYSROOT, &[Src, Phase(PkgSetup)]),
            (ESYSROOT, &[Src, Phase(PkgSetup)]),
            (BROOT, &[Src, Phase(PkgSetup)]),
        ])
        .disable_env(&[PORTDIR, ECLASSDIR, DESTTREE, INSDESTTREE])
        .update_hooks(&HOOKS)
        .finalize()
});

pub static EAPI8: Lazy<Eapi> = Lazy::new(|| {
    use Feature::*;

    Eapi::new("8", Some(&EAPI7))
        .enable_features(&[ConsistentFileOpts, DosymRelative, SrcUriUnrestrict, UsevTwoArgs])
        .update_dep_keys(&[Key::Idepend])
        .update_incremental_keys(&[Key::Idepend, Key::Properties, Key::Restrict])
        .update_econf(&[
            ("--datarootdir", None, Some("${EPREFIX}/usr/share")),
            ("--disable-static", Some(&["--disable-static", "--enable-static"]), None),
        ])
        .disable_archives(&["7z", "7Z", "rar", "RAR", "LHA", "LHa", "lha", "lzh"])
        .finalize()
});

/// Reference to the most recent, official EAPI.
pub static EAPI_LATEST_OFFICIAL: Lazy<&'static Eapi> = Lazy::new(|| &EAPI8);

/// The latest EAPI with extensions on top.
pub static EAPI_PKGCRAFT: Lazy<Eapi> = Lazy::new(|| {
    use Feature::*;
    Eapi::new("pkgcraft", Some(&EAPI_LATEST_OFFICIAL))
        .enable_features(&[RepoIds])
        .finalize()
});

/// Reference to the most recent EAPI.
pub static EAPI_LATEST: Lazy<&'static Eapi> = Lazy::new(|| &EAPI_PKGCRAFT);

/// Ordered set of official EAPIs.
pub static EAPIS_OFFICIAL: Lazy<IndexSet<&'static Eapi>> = Lazy::new(|| {
    let mut eapis = IndexSet::new();
    let mut eapi: &Eapi = &EAPI_LATEST_OFFICIAL;
    while let Some(x) = eapi.parent {
        eapis.insert(eapi);
        eapi = x;
    }
    eapis.insert(eapi);
    // reverse so it's in chronological order
    eapis.reverse();
    eapis
});

/// Ordered set of unofficial EAPIs.
pub static EAPIS_UNOFFICIAL: Lazy<IndexSet<&'static Eapi>> = Lazy::new(|| {
    let mut eapis = IndexSet::new();
    let mut eapi: &Eapi = &EAPI_LATEST;
    while let Some(x) = eapi.parent {
        eapis.insert(eapi);
        if EAPIS_OFFICIAL.contains(x) {
            break;
        } else {
            eapi = x;
        }
    }
    // reverse so it's in chronological order
    eapis.reverse();
    eapis
});

/// Ordered set of EAPIs.
pub static EAPIS: Lazy<IndexSet<&'static Eapi>> = Lazy::new(|| {
    EAPIS_OFFICIAL
        .iter()
        .chain(EAPIS_UNOFFICIAL.iter())
        .copied()
        .collect()
});

/// Convert EAPI range into an iterator of EAPIs.
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

    let eapis = if inclusive {
        Either::Left((start..=end).map(|n| EAPIS[n]))
    } else {
        Either::Right((start..end).map(|n| EAPIS[n]))
    };

    Ok(eapis)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Restrict {
    Id(StrRestrict),
    Has(Feature),
}

impl Restriction<&'static Eapi> for Restrict {
    fn matches(&self, eapi: &'static Eapi) -> bool {
        use Restrict::*;
        match self {
            Id(r) => r.matches(eapi.as_str()),
            Has(feature) => eapi.has(*feature),
        }
    }
}

#[cfg(test)]
mod tests {
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
        assert!(*EAPI0 < **EAPI_LATEST_OFFICIAL);
        assert!(*EAPI0 <= *EAPI0);
        assert!(*EAPI0 == *EAPI0);
        assert!(*EAPI0 >= *EAPI0);
        assert!(**EAPI_LATEST_OFFICIAL > *EAPI0);
        assert!(**EAPI_LATEST > **EAPI_LATEST_OFFICIAL);
    }

    #[test]
    fn test_has() {
        assert!(!EAPI0.has(Feature::UseDeps));
        assert!(EAPI_LATEST_OFFICIAL.has(Feature::UseDeps));
    }

    #[test]
    fn test_dep_parsing() {
        let dep = EAPI0.dep("cat/pkg").unwrap();
        assert_eq!(dep.category(), "cat");
        assert_eq!(dep.package(), "pkg");
        assert_eq!(dep.to_string(), "cat/pkg");

        let dep = EAPI1.dep("cat/pkg:0").unwrap();
        assert_eq!(dep.category(), "cat");
        assert_eq!(dep.package(), "pkg");
        assert_eq!(dep.slot().unwrap(), "0");
        assert_eq!(dep.to_string(), "cat/pkg:0");

        let r = EAPI0.dep("cat/pkg:0");
        assert_err_re!(r, "invalid dep: cat/pkg:0");
        let r = EAPI_LATEST_OFFICIAL.dep("cat/pkg::repo");
        assert_err_re!(r, "invalid dep: cat/pkg::repo");
        let dep = EAPI_LATEST.dep("cat/pkg::repo").unwrap();
        assert_eq!(dep.repo().unwrap(), "repo");
    }

    #[test]
    fn test_try_from() {
        assert_eq!(&*EAPI0, TryInto::<&'static Eapi>::try_into(&*EAPI0).unwrap());
        assert_eq!(&*EAPI1, TryInto::<&'static Eapi>::try_into("1").unwrap());

        let mut arg: Option<&str> = None;
        let mut eapi: &Eapi;
        eapi = arg.try_into().unwrap();
        assert_eq!(&*EAPI_PKGCRAFT, eapi);
        arg = Some("1");
        eapi = arg.try_into().unwrap();
        assert_eq!(&*EAPI1, eapi);

        let mut arg: Option<&'static Eapi> = None;
        eapi = arg.try_into().unwrap();
        assert_eq!(&*EAPI_PKGCRAFT, eapi);
        arg = Some(&EAPI1);
        eapi = arg.try_into().unwrap();
        assert_eq!(&*EAPI1, eapi);
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
