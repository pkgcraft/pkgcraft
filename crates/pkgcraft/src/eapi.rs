use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::LazyLock;
use std::{fmt, fs, io};

use camino::Utf8Path;
use indexmap::IndexSet;
use itertools::Either;
use strum::EnumString;

use crate::archive::Archive;
use crate::dep;
use crate::pkg::ebuild::metadata::Key;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::Restriction;
use crate::shell::commands::Command;
use crate::shell::environment::{BuildVariable, Variable};
use crate::shell::hooks::{Hook, HookKind};
use crate::shell::operations::{Operation, OperationKind};
use crate::shell::phase::{Phase, PhaseKind};
use crate::Error;

peg::parser!(grammar parse() for str {
    // EAPIs must not begin with a hyphen, dot, or plus sign.
    pub(super) rule eapi() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
        } / expected!("EAPI"))
        { s }

    rule single_quotes<T>(expr: rule<T>) -> T
        = "\'" v:expr() "\'" { v }

    rule double_quotes<T>(expr: rule<T>) -> T
        = "\"" v:expr() "\"" { v }

    rule optionally_quoted<T>(expr: rule<T>) -> T
        = s:expr() { s }
        / s:double_quotes(<expr()>) { s }
        / s:single_quotes(<expr()>) { s }

    pub(super) rule eapi_value() -> &'input str
        = s:optionally_quoted(<eapi()>) { s }
});

pub(crate) fn parse_value(s: &str) -> crate::Result<&str> {
    parse::eapi_value(s).map_err(|_| Error::InvalidValue(format!("invalid EAPI: {s:?}")))
}

/// Features that relate to differentiation between EAPIs as specified by PMS.
#[derive(EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum Feature {
    // EAPI 5
    /// `best_version` and `has_version` support --host-root
    QueryHostRoot,

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
    /// `domo` uses DESTTREE for destination paths
    DomoUsesDesttree,
    /// `best_version` and `has_version` support -b/-d/-r options
    QueryDeps,
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
    features: HashSet<Feature>,
    operations: HashSet<Operation>,
    phases: IndexSet<Phase>,
    dep_keys: IndexSet<Key>,
    incremental_keys: IndexSet<Key>,
    mandatory_keys: IndexSet<Key>,
    metadata_keys: IndexSet<Key>,
    econf_options: EapiEconfOptions,
    archives: IndexSet<String>,
    env: HashSet<BuildVariable>,
    commands: HashSet<Command>,
    hooks: HashMap<PhaseKind, HashMap<HookKind, IndexSet<Hook>>>,
}

impl PartialEq for Eapi {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Eapi {}

impl Hash for Eapi {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Borrow<str> for &'static Eapi {
    fn borrow(&self) -> &str {
        &self.id
    }
}

impl Ord for Eapi {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_index = EAPIS.get_index_of(self).unwrap();
        let other_index = EAPIS.get_index_of(other).unwrap();
        self_index.cmp(&other_index)
    }
}

impl PartialOrd for Eapi {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Eapi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl fmt::Debug for Eapi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Eapi {{ id: {} }}", self.id)
    }
}

impl FromStr for &'static Eapi {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        if let Some(eapi) = EAPIS.get(s) {
            Ok(eapi)
        } else if Eapi::parse(s).is_ok() {
            Err(Error::InvalidValue(format!("unsupported EAPI: {s}")))
        } else {
            Err(Error::InvalidValue(format!("invalid EAPI: {s:?}")))
        }
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
        value.parse()
    }
}

impl TryFrom<Option<&str>> for &'static Eapi {
    type Error = Error;

    fn try_from(value: Option<&str>) -> crate::Result<&'static Eapi> {
        value.map_or_else(|| Ok(Default::default()), |s| s.parse())
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
            Ok(s) => s.trim_end().parse(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                Err(Error::InvalidValue("unsupported EAPI: 0".to_string()))
            }
            Err(e) => Err(Error::IO(format!("failed reading EAPI: {value}: {e}"))),
        }
    }
}

impl Eapi {
    /// Create a new Eapi given an identifier and optional Eapi to inherit from.
    fn new(id: &str, eapi: Option<&'static Eapi>) -> Self {
        let mut eapi = eapi.cloned().unwrap_or_default();
        eapi.id = id.to_string();
        eapi
    }

    /// Verify a string represents a valid EAPI.
    pub fn parse<S: AsRef<str>>(s: S) -> crate::Result<()> {
        let s = s.as_ref();
        parse::eapi(s).map_err(|_| Error::InvalidValue(format!("invalid EAPI: {s:?}")))?;
        Ok(())
    }

    /// Return the ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.id
    }

    /// Check if an EAPI has a given feature.
    pub fn has(&self, feature: Feature) -> bool {
        self.features.contains(&feature)
    }

    /// Parse a package dependency using a specific EAPI.
    pub fn dep<S: AsRef<str>>(&'static self, s: S) -> crate::Result<dep::Dep> {
        dep::parse::dep(s.as_ref(), self)
    }

    /// Return the ordered set of phases for a given operation.
    pub(crate) fn operation(&self, op: OperationKind) -> crate::Result<&Operation> {
        self.operations.get(&op).ok_or_else(|| {
            Error::InvalidValue(format!("EAPI {self}: unknown operation: {op}"))
        })
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

        // determine if an archive path has a matching file extension
        let matches = |ext: &str| -> bool {
            if self.has(Feature::UnpackCaseInsensitive) {
                let ext = format!(".{}", ext.to_lowercase());
                path.as_str().to_lowercase().ends_with(&ext)
            } else {
                let ext = format!(".{ext}");
                path.as_str().ends_with(&ext)
            }
        };

        self.archives
            .iter()
            .find(|ext| matches(ext))
            .ok_or_else(|| Error::InvalidValue(format!("unknown archive format: {path}")))
            .and_then(|ext| Archive::from_path(path).map(|x| (ext.as_str(), x)))
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
    pub fn mandatory_keys(&self) -> &IndexSet<Key> {
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

    /// Return the set of all environment variables.
    pub(crate) fn env(&self) -> &HashSet<BuildVariable> {
        &self.env
    }

    /// Return all the enabled commands for an EAPI.
    pub fn commands(&self) -> &HashSet<Command> {
        &self.commands
    }

    /// Return the hooks for a given Phase.
    pub(crate) fn hooks(&self) -> &HashMap<PhaseKind, HashMap<HookKind, IndexSet<Hook>>> {
        &self.hooks
    }

    /// Enable commands during Eapi registration.
    fn update_commands<I>(mut self, commands: I) -> Self
    where
        I: IntoIterator<Item = Command>,
    {
        self.commands.extend(commands);
        self
    }

    /// Disable inherited commands during Eapi registration.
    fn disable_commands<I>(mut self, commands: I) -> Self
    where
        I: IntoIterator<Item = scallop::builtins::Builtin>,
    {
        for b in commands {
            if !self.commands.remove(&b) {
                unreachable!("EAPI {self}: disabling unset builtin: {b}");
            }
        }
        self
    }

    /// Enable features during Eapi registration.
    fn enable_features<I>(mut self, features: I) -> Self
    where
        I: IntoIterator<Item = Feature>,
    {
        for f in features {
            if !self.features.insert(f) {
                unreachable!("EAPI {self}: enabling set feature: {f:?}");
            }
        }
        self
    }

    /// Disable inherited features during Eapi registration.
    fn disable_features<I>(mut self, features: I) -> Self
    where
        I: IntoIterator<Item = Feature>,
    {
        for f in features {
            if !self.features.remove(&f) {
                unreachable!("EAPI {self}: disabling unset feature: {f:?}");
            }
        }
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
                    if op.phases.contains(phase) {
                        op.phases.replace(*phase);
                    }
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
        self
    }

    /// Disable support for archive extensions during Eapi registration.
    fn disable_archives(mut self, types: &[&str]) -> Self {
        for x in types {
            if !self.archives.swap_remove(*x) {
                unreachable!("EAPI {self}: disabling unknown archive format: {x:?}");
            }
        }
        self
    }

    /// Enable support for build variables during Eapi registration.
    fn update_env<I>(mut self, variables: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<BuildVariable>,
    {
        for var in variables {
            self.env.replace(var.into());
        }
        self
    }

    /// Disable support for build variables during Eapi registration.
    fn disable_env<I: IntoIterator<Item = Variable>>(mut self, variables: I) -> Self {
        for var in variables {
            if !self.env.remove(&var) {
                unreachable!("EAPI {self}: disabling unregistered variable: {var}");
            }
        }
        self
    }

    /// Update hooks during Eapi registration.
    fn update_hooks<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = Hook>,
    {
        for hook in values {
            let hooks = self
                .hooks
                .entry(hook.phase)
                .or_default()
                .entry(hook.kind)
                .or_default();
            hooks.insert(hook);
            hooks.sort();
        }
        self
    }

    /// Finalize and sort ordered fields that depend on previous operations.
    fn finalize(mut self) -> Self {
        self.phases = self.operations.iter().flatten().copied().collect();
        // sort phases by name
        self.phases.sort();
        // sort archives by extension length, longest to shortest.
        self.archives.sort_by(|s1, s2| (s2.len().cmp(&s1.len())));
        self
    }
}

static OLD_EAPIS: LazyLock<IndexSet<String>> = LazyLock::new(|| {
    let end = EAPIS_OFFICIAL[0]
        .id
        .parse()
        .expect("non-integer based EAPI");
    (0..end).map(|s| s.to_string()).collect()
});

pub static EAPI5: LazyLock<Eapi> = LazyLock::new(|| {
    use crate::pkg::ebuild::metadata::Key::*;
    use crate::shell::commands::*;
    use crate::shell::environment::Variable::*;
    use crate::shell::hooks::*;
    use crate::shell::operations::OperationKind::*;
    use crate::shell::phase::{eapi5::*, PhaseKind::*};
    use crate::shell::scope::EbuildScope::*;
    use Feature::*;

    Eapi::new("5", None)
        .enable_features([DomoUsesDesttree, QueryHostRoot, TrailingSlash])
        .update_commands([
            Command::new(adddeny, [Phases]),
            Command::new(addpredict, [Phases]),
            Command::new(addread, [Phases]),
            Command::new(addwrite, [Phases]),
            Command::new(assert, [All]),
            Command::new(best_version, [Phases]),
            Command::new(command_not_found_handle, [All]),
            Command::new(debug_print, [All]),
            Command::new(debug_print_function, [All]),
            Command::new(debug_print_section, [All]),
            Command::new(default, [Phases]),
            Command::new(default_pkg_nofetch, [PkgNofetch]),
            Command::new(default_src_compile, [SrcCompile]),
            Command::new(default_src_configure, [SrcConfigure]),
            Command::new(default_src_install, [SrcInstall]),
            Command::new(default_src_prepare, [SrcPrepare]),
            Command::new(default_src_test, [SrcTest]),
            Command::new(default_src_unpack, [SrcUnpack]),
            Command::new(die, [All]),
            Command::new(diropts, [SrcInstall]),
            Command::new(dobin, [SrcInstall]),
            Command::new(docinto, [SrcInstall]),
            Command::new(docompress, [SrcInstall]),
            Command::new(doconfd, [SrcInstall]),
            Command::new(dodir, [SrcInstall]),
            Command::new(dodoc, [SrcInstall]),
            Command::new(doenvd, [SrcInstall]),
            Command::new(doexe, [SrcInstall]),
            Command::new(doheader, [SrcInstall]),
            Command::new(dohtml, [SrcInstall]),
            Command::new(doinfo, [SrcInstall]),
            Command::new(doinitd, [SrcInstall]),
            Command::new(doins, [SrcInstall]),
            Command::new(dolib, [SrcInstall]),
            Command::new(dolib_a, [SrcInstall]),
            Command::new(dolib_so, [SrcInstall]),
            Command::new(doman, [SrcInstall]),
            Command::new(domo, [SrcInstall]),
            Command::new(dosbin, [SrcInstall]),
            Command::new(dosym, [SrcInstall]),
            Command::new(ebegin, [Phases]),
            Command::new(econf, [SrcConfigure]),
            Command::new(eend, [Phases]),
            Command::new(eerror, [Phases]),
            Command::new(einfo, [Phases]),
            Command::new(einfon, [Phases]),
            Command::new(einstall, [SrcInstall]),
            Command::new(elog, [Phases]),
            Command::new(emake, [Phases]),
            Command::new(ewarn, [Phases]),
            Command::new(exeinto, [SrcInstall]),
            Command::new(exeopts, [SrcInstall]),
            Command::new(export_functions, [Eclass]),
            Command::new(fowners, [SrcInstall, PkgPreinst, PkgPostinst]),
            Command::new(fperms, [SrcInstall, PkgPreinst, PkgPostinst]),
            Command::new(has, [All]),
            Command::new(has_version, [Phases]),
            Command::new(hasq, [All]),
            Command::new(hasv, [All]),
            Command::new(inherit, [Global, Eclass]),
            Command::new(insinto, [SrcInstall]),
            Command::new(insopts, [SrcInstall]),
            Command::new(into, [SrcInstall]),
            Command::new(keepdir, [SrcInstall]),
            Command::new(libopts, [SrcInstall]),
            Command::new(newbin, [SrcInstall]),
            Command::new(newconfd, [SrcInstall]),
            Command::new(newdoc, [SrcInstall]),
            Command::new(newenvd, [SrcInstall]),
            Command::new(newexe, [SrcInstall]),
            Command::new(newheader, [SrcInstall]),
            Command::new(newinitd, [SrcInstall]),
            Command::new(newins, [SrcInstall]),
            Command::new(newlib_a, [SrcInstall]),
            Command::new(newlib_so, [SrcInstall]),
            Command::new(newman, [SrcInstall]),
            Command::new(newsbin, [SrcInstall]),
            Command::new(nonfatal, [All]),
            Command::new(unpack, [Phases]),
            Command::new(use_, [Phases]),
            Command::new(use_enable, [Phases]),
            Command::new(use_with, [Phases]),
            Command::new(useq, [Phases]),
            Command::new(usev, [Phases]),
            Command::new(usex, [Phases]),
            // phase stubs that force direct calls to error out
            Command::new(pkg_config_stub, [All]),
            Command::new(pkg_info_stub, [All]),
            Command::new(pkg_nofetch_stub, [All]),
            Command::new(pkg_postinst_stub, [All]),
            Command::new(pkg_postrm_stub, [All]),
            Command::new(pkg_preinst_stub, [All]),
            Command::new(pkg_prerm_stub, [All]),
            Command::new(pkg_pretend_stub, [All]),
            Command::new(pkg_setup_stub, [All]),
            Command::new(src_compile_stub, [All]),
            Command::new(src_configure_stub, [All]),
            Command::new(src_install_stub, [All]),
            Command::new(src_prepare_stub, [All]),
            Command::new(src_test_stub, [All]),
            Command::new(src_unpack_stub, [All]),
        ])
        .update_operations([
            Pretend.phase(PkgPretend),
            Build
                .phase(PkgSetup)
                .phase(SrcUnpack.func(src_unpack))
                .phase(SrcPrepare)
                .phase(SrcConfigure.func(src_configure))
                .phase(SrcCompile.func(src_compile))
                .phase(SrcTest.func(src_test))
                .phase(SrcInstall.func(src_install)),
            Install.phases([PkgPreinst, PkgPostinst]),
            Uninstall.phases([PkgPrerm, PkgPostrm]),
            Replace.phases([PkgPreinst, PkgPrerm, PkgPostrm, PkgPostinst]),
            Config.phase(PkgConfig),
            Info.phase(PkgInfo),
            NoFetch.phase(PkgNofetch.func(pkg_nofetch)),
        ])
        .update_dep_keys(&[DEPEND, RDEPEND, PDEPEND])
        .update_incremental_keys(&[IUSE, DEPEND, RDEPEND, PDEPEND, REQUIRED_USE])
        .update_mandatory_keys(&[DESCRIPTION, EAPI, SLOT])
        .update_metadata_keys(&[
            CHKSUM,
            DEFINED_PHASES,
            HOMEPAGE,
            INHERIT,
            INHERITED,
            IUSE,
            KEYWORDS,
            LICENSE,
            PROPERTIES,
            REQUIRED_USE,
            RESTRICT,
            SRC_URI,
        ])
        .enable_archives(&[
            "tar", "gz", "Z", "tar.gz", "tgz", "tar.Z", "bz2", "bz", "tar.bz2", "tbz2",
            "tar.bz", "tbz", "zip", "ZIP", "jar", "7z", "7Z", "rar", "RAR", "LHA", "LHa",
            "lha", "lzh", "a", "deb", "lzma", "tar.lzma", "tar.xz", "xz",
        ])
        .update_env([
            P.internal([All]),
            PF.internal([All]),
            PN.internal([All]),
            CATEGORY.internal([All]),
            PV.internal([All]),
            PR.internal([All]),
            PVR.internal([All]),
            A.internal([Src, Phase(PkgNofetch)]),
            FILESDIR.internal([Src, Global]),
            DISTDIR.internal([Src, Global]),
            WORKDIR.internal([Src, Global]),
            S.internal([Src]),
            PORTDIR.internal([Src]),
            ECLASSDIR.internal([Src]),
            ROOT.internal([Pkg]),
            T.internal([All]),
            TMPDIR.internal([All]).external(),
            HOME.internal([All]).external(),
            D.internal([SrcInstall, PkgPreinst, PkgPostinst]),
            DESTTREE.internal([SrcInstall]),
            INSDESTTREE.internal([SrcInstall]),
            USE.internal([All]),
            EBUILD_PHASE.internal([Phases]),
            EBUILD_PHASE_FUNC.internal([Phases]),
            EPREFIX.internal([Global]),
            ED.internal([SrcInstall, PkgPreinst, PkgPostinst]),
            EROOT.internal([Pkg]),
            MERGE_TYPE.internal([Pkg]),
            REPLACING_VERSIONS.internal([Pkg]),
            REPLACED_BY_VERSION.internal([PkgPrerm, PkgPostrm]),
        ])
        // unexported, internal variables
        .update_env([DOCDESTTREE, EXEDESTTREE])
        .update_econf(&[
            ("--disable-dependency-tracking", None, None),
            ("--disable-silent-rules", None, None),
        ])
        .update_hooks([
            SrcInstall.pre("docompress", docompress::pre),
            SrcInstall.post("docompress", docompress::post),
        ])
        .finalize()
});

pub static EAPI6: LazyLock<Eapi> = LazyLock::new(|| {
    use crate::shell::commands::*;
    use crate::shell::phase::{eapi6::*, PhaseKind::*};
    use crate::shell::scope::EbuildScope::*;
    use Feature::*;

    Eapi::new("6", Some(&EAPI5))
        .enable_features([
            NonfatalDie,
            GlobalFailglob,
            UnpackExtendedPath,
            UnpackCaseInsensitive,
        ])
        .update_commands([
            Command::new(eapply, [SrcPrepare]),
            Command::new(eapply_user, [SrcPrepare]),
            Command::new(einstalldocs, [SrcInstall]),
            Command::new(get_libdir, [All]),
            Command::new(in_iuse, [Phases]),
        ])
        .disable_commands([einstall])
        .update_phases([SrcPrepare.func(src_prepare), SrcInstall.func(src_install)])
        .update_econf(&[
            ("--docdir", None, Some("${EPREFIX}/usr/share/doc/${PF}")),
            ("--htmldir", None, Some("${EPREFIX}/usr/share/doc/${PF}/html")),
        ])
        .enable_archives(&["txz"])
        .finalize()
});

pub static EAPI7: LazyLock<Eapi> = LazyLock::new(|| {
    use crate::pkg::ebuild::metadata::Key::*;
    use crate::shell::commands::*;
    use crate::shell::environment::Variable::*;
    use crate::shell::hooks::*;
    use crate::shell::phase::PhaseKind::*;
    use crate::shell::scope::EbuildScope::*;
    use Feature::*;

    Eapi::new("7", Some(&EAPI6))
        .enable_features([QueryDeps])
        .disable_features([DomoUsesDesttree, QueryHostRoot, TrailingSlash])
        .update_commands([
            Command::new(dostrip, [SrcInstall]),
            Command::new(eqawarn, [Phases]),
            Command::new(ver_cut, [All]),
            Command::new(ver_rs, [All]),
            Command::new(ver_test, [All]),
        ])
        .disable_commands([dohtml, dolib, libopts])
        .update_dep_keys(&[BDEPEND])
        .update_incremental_keys(&[BDEPEND])
        .update_econf(&[("--with-sysroot", None, Some("${ESYSROOT:-/}"))])
        .update_env([
            SYSROOT.internal([Src, Phase(PkgSetup)]),
            ESYSROOT.internal([Src, Phase(PkgSetup)]),
            BROOT.internal([Src, Phase(PkgSetup)]),
        ])
        // unexported, internal variables
        .update_env([DESTTREE, INSDESTTREE])
        // entirely removed
        .disable_env([PORTDIR, ECLASSDIR])
        .update_hooks([
            SrcInstall.pre("dostrip", dostrip::pre),
            SrcInstall.post("dostrip", dostrip::post),
        ])
        .finalize()
});

pub static EAPI8: LazyLock<Eapi> = LazyLock::new(|| {
    use crate::pkg::ebuild::metadata::Key::*;
    use crate::shell::commands::*;
    use Feature::*;

    Eapi::new("8", Some(&EAPI7))
        .enable_features([ConsistentFileOpts, DosymRelative, SrcUriUnrestrict, UsevTwoArgs])
        .disable_commands([hasq, hasv, useq])
        .update_dep_keys(&[IDEPEND])
        .update_incremental_keys(&[IDEPEND, PROPERTIES, RESTRICT])
        .update_econf(&[
            ("--datarootdir", None, Some("${EPREFIX}/usr/share")),
            ("--disable-static", Some(&["--disable-static", "--enable-static"]), None),
        ])
        .disable_archives(&["7z", "7Z", "rar", "RAR", "LHA", "LHa", "lha", "lzh"])
        .finalize()
});

/// Reference to the most recent, official EAPI.
pub static EAPI_LATEST_OFFICIAL: LazyLock<&'static Eapi> = LazyLock::new(|| &EAPI8);

/// The latest EAPI with extensions on top.
pub static EAPI_PKGCRAFT: LazyLock<Eapi> = LazyLock::new(|| {
    use Feature::*;
    Eapi::new("pkgcraft", Some(&EAPI_LATEST_OFFICIAL))
        .enable_features([RepoIds])
        .finalize()
});

/// Reference to the most recent EAPI.
pub static EAPI_LATEST: LazyLock<&'static Eapi> = LazyLock::new(|| &EAPI_PKGCRAFT);

/// Ordered set of official, supported EAPIs.
pub static EAPIS_OFFICIAL: LazyLock<IndexSet<&'static Eapi>> =
    LazyLock::new(|| [&*EAPI5, &*EAPI6, &*EAPI7, &*EAPI8].into_iter().collect());

/// Ordered set of unofficial EAPIs.
pub static EAPIS_UNOFFICIAL: LazyLock<IndexSet<&'static Eapi>> =
    LazyLock::new(|| [&*EAPI_PKGCRAFT].into_iter().collect());

/// Ordered set of EAPIs.
pub static EAPIS: LazyLock<IndexSet<&'static Eapi>> = LazyLock::new(|| {
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
        "U" => Ok(EAPIS.get_index_of(EAPIS_UNOFFICIAL[0]).unwrap()),
        _ => {
            if let Some(idx) = EAPIS.get_index_of(s) {
                Ok(idx)
            } else if OLD_EAPIS.contains(s) {
                // EAPI has been removed so use the oldest, supported EAPI
                Ok(0)
            } else {
                Err(err())
            }
        }
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
        match self {
            Self::Id(r) => r.matches(&eapi.id),
            Self::Has(feature) => eapi.has(*feature),
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use crate::test::assert_err_re;
    use crate::test::assert_ordered_eq;

    use super::*;

    #[test]
    fn parse() {
        assert!(Eapi::parse("1.2.3").is_ok());
        assert!(Eapi::parse("_").is_ok());
        assert!(Eapi::parse("+5").is_err());
        assert!(Eapi::parse(".1").is_err());
    }

    #[test]
    fn from_str() {
        assert!(<&Eapi>::from_str("-invalid").is_err());
        assert!(<&Eapi>::from_str("unknown").is_err());
        assert_eq!(<&Eapi>::from_str("8").unwrap(), &*EAPI8);
    }

    #[test]
    fn display_and_debug() {
        for eapi in &*EAPIS {
            let s = eapi.to_string();
            assert!(format!("{eapi:?}").contains(&s));
        }
    }

    #[test]
    fn cmp() {
        assert!(**EAPI_LATEST_OFFICIAL < **EAPI_LATEST);
        assert!(*EAPI8 <= *EAPI8);
        assert!(*EAPI8 == *EAPI8);
        assert!(*EAPI8 >= *EAPI8);
        assert!(**EAPI_LATEST > **EAPI_LATEST_OFFICIAL);
    }

    #[test]
    fn has() {
        assert!(!EAPI_LATEST_OFFICIAL.has(Feature::RepoIds));
        assert!(EAPI_LATEST_OFFICIAL.has(Feature::UsevTwoArgs));
    }

    #[test]
    fn dep_parsing() {
        let dep = EAPI_LATEST_OFFICIAL.dep("cat/pkg").unwrap();
        assert_eq!(dep.category(), "cat");
        assert_eq!(dep.package(), "pkg");
        assert_eq!(dep.to_string(), "cat/pkg");

        let dep = EAPI_LATEST_OFFICIAL.dep("cat/pkg:0").unwrap();
        assert_eq!(dep.category(), "cat");
        assert_eq!(dep.package(), "pkg");
        assert_eq!(dep.slot().unwrap(), "0");
        assert_eq!(dep.to_string(), "cat/pkg:0");

        let r = EAPI_LATEST_OFFICIAL.dep("cat/pkg::repo");
        assert_err_re!(r, "invalid dep: cat/pkg::repo");
        let dep = EAPI_LATEST.dep("cat/pkg::repo").unwrap();
        assert_eq!(dep.repo().unwrap(), "repo");
    }

    #[test]
    fn try_from() {
        let mut eapi: &Eapi;

        // &str
        eapi = "8".try_into().unwrap();
        assert_eq!(&*EAPI8, eapi);

        // Option<&str>
        let mut arg: Option<&str> = None;
        eapi = arg.try_into().unwrap();
        assert_eq!(&*EAPI_PKGCRAFT, eapi);
        arg = Some("8");
        eapi = arg.try_into().unwrap();
        assert_eq!(&*EAPI8, eapi);

        // Option<&Eapi>
        let mut arg: Option<&'static Eapi> = None;
        eapi = arg.try_into().unwrap();
        assert_eq!(&*EAPI_PKGCRAFT, eapi);
        arg = Some(&EAPI8);
        eapi = arg.try_into().unwrap();
        assert_eq!(&*EAPI8, eapi);

        // &Utf8Path
        let r: crate::Result<&Eapi> = Utf8Path::new("nonexistent").try_into();
        assert_err_re!(r, "unsupported EAPI: 0$");
        let dir = tempfile::tempdir().unwrap();
        let r: crate::Result<&Eapi> = Utf8Path::new(dir.path().to_str().unwrap()).try_into();
        assert_err_re!(r, "failed reading EAPI: ");
        let file = NamedTempFile::new().unwrap();
        fs::write(&file, "8").unwrap();
        eapi = Utf8Path::new(file.path().to_str().unwrap())
            .try_into()
            .unwrap();
        assert_eq!(&*EAPI8, eapi);
    }

    #[test]
    fn eapi_range() {
        // invalid
        for s in ["", "1", "1..=", "..=", "...", "0-", "-1..", "1..9999", "..=unknown"] {
            let r = range(s);
            assert!(r.is_err(), "range didn't fail: {s}");
        }

        // removed EAPIs
        assert!(range("2..3").unwrap().next().is_none());
        assert!(range("..5").unwrap().next().is_none());

        // existing EAPIs
        assert_ordered_eq!(range("..").unwrap(), EAPIS.iter().copied());
        assert_ordered_eq!(range("..U").unwrap(), EAPIS_OFFICIAL.iter().copied());
        assert_ordered_eq!(range("U..").unwrap(), EAPIS_UNOFFICIAL.iter().copied());
        assert!(range("8..8").unwrap().next().is_none());
        assert_ordered_eq!(range("7..8").unwrap(), [&*EAPI7]);
        assert_ordered_eq!(range("7..=8").unwrap(), [&*EAPI7, &*EAPI8]);
        assert_ordered_eq!(range("..6").unwrap(), [&*EAPI5]);
        assert_ordered_eq!(range("..=6").unwrap(), [&*EAPI5, &*EAPI6]);
    }
}
