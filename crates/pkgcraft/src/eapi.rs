use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::LazyLock;
use std::{fmt, fs, io};

use camino::Utf8Path;
use indexmap::{IndexSet, set::MutableValues};
use itertools::Either;
use strum::EnumString;

use crate::Error;
use crate::archive::Archive;
use crate::dep;
use crate::pkg::ebuild::MetadataKey;
use crate::restrict::Restriction;
use crate::restrict::str::Restrict as StrRestrict;
use crate::shell::commands::econf::EconfOption;
use crate::shell::commands::{Builtin, Command};
use crate::shell::environment::{BuildVariable, Variable};
use crate::shell::hooks::HookBuilder;
use crate::shell::operations::{Operation, OperationKind};
use crate::shell::phase::Phase;

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

/// EAPI object.
#[derive(Default, Clone)]
pub struct Eapi {
    id: String,
    features: HashSet<Feature>,
    operations: HashSet<Operation>,
    phases: IndexSet<Phase>,
    dep_keys: IndexSet<MetadataKey>,
    incremental_keys: IndexSet<MetadataKey>,
    mandatory_keys: IndexSet<MetadataKey>,
    metadata_keys: IndexSet<MetadataKey>,
    econf_options: IndexSet<EconfOption>,
    archives: HashSet<String>,
    env: IndexSet<BuildVariable>,
    commands: IndexSet<Command>,
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

impl AsRef<str> for Eapi {
    fn as_ref(&self) -> &str {
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
    #[allow(dead_code)]
    pub(crate) fn operation(&self, op: OperationKind) -> impl Iterator<Item = &Phase> {
        self.operations
            .get(&op)
            .unwrap_or_else(|| panic!("EAPI {self}: unknown operation: {op}"))
            .into_iter()
            .map(move |phase| {
                self.phases
                    .get(phase)
                    .unwrap_or_else(|| panic!("EAPI {self}: unregistered phase: {phase}"))
            })
    }

    /// Return all the known phases for an EAPI.
    pub(crate) fn phases(&self) -> &IndexSet<Phase> {
        &self.phases
    }

    /// Load an archive from a given path if it's supported.
    pub(crate) fn archive_from_path<P>(&self, path: P) -> crate::Result<Archive>
    where
        P: AsRef<Utf8Path>,
    {
        let archive = Archive::from_path(path)?;
        let file_name = archive.file_name();
        let ext = archive.ext();

        if !self.has(Feature::UnpackCaseInsensitive) && !file_name.ends_with(ext) {
            Err(Error::InvalidValue(format!("unmatched archive extension: {file_name}")))
        } else if self.archives.contains(ext) {
            Ok(archive)
        } else {
            Err(Error::InvalidValue(format!("unsupported archive format: {file_name}")))
        }
    }

    /// Metadata variables for dependencies.
    pub fn dep_keys(&self) -> &IndexSet<MetadataKey> {
        &self.dep_keys
    }

    /// Metadata variables that are incrementally handled.
    pub(crate) fn incremental_keys(&self) -> &IndexSet<MetadataKey> {
        &self.incremental_keys
    }

    /// Metadata variables that must exist.
    pub fn mandatory_keys(&self) -> &IndexSet<MetadataKey> {
        &self.mandatory_keys
    }

    /// Metadata variables that may exist.
    pub fn metadata_keys(&self) -> &IndexSet<MetadataKey> {
        &self.metadata_keys
    }

    /// Return all EAPI-specific econf options.
    pub(crate) fn econf_options(&self) -> &IndexSet<EconfOption> {
        &self.econf_options
    }

    /// Return the set of all environment variables.
    pub fn env(&self) -> &IndexSet<BuildVariable> {
        &self.env
    }

    /// Return all the enabled commands for an EAPI.
    pub fn commands(&self) -> &IndexSet<Command> {
        &self.commands
    }

    /// Enable commands during Eapi registration.
    fn update_commands<I>(mut self, commands: I) -> Self
    where
        I: IntoIterator<Item = Command>,
    {
        self.commands.extend(commands);
        self.commands.sort_unstable();
        self
    }

    /// Disable inherited commands during Eapi registration.
    fn disable_commands<I>(mut self, commands: I) -> Self
    where
        I: IntoIterator<Item = Builtin>,
    {
        for b in commands {
            if !self.commands.swap_remove(&b) {
                unreachable!("EAPI {self}: disabling unset command: {b}");
            }
        }
        self.commands.sort_unstable();
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
        for operation in operations {
            self.operations.replace(operation);
        }
        self
    }

    /// Update phases for all known operations during Eapi registration.
    fn update_phases<I>(mut self, phases: I) -> Self
    where
        I: IntoIterator<Item = Phase>,
    {
        for phase in phases {
            self.phases.replace(phase);
        }
        self.phases.sort_unstable();
        self
    }

    /// Update dependency types during Eapi registration.
    fn update_dep_keys(mut self, updates: &[MetadataKey]) -> Self {
        self.dep_keys.extend(updates);
        self.dep_keys.sort_unstable();
        self.metadata_keys.extend(updates);
        self.metadata_keys.sort_unstable();
        self
    }

    /// Update incremental variables during Eapi registration.
    fn update_incremental_keys(mut self, updates: &[MetadataKey]) -> Self {
        self.incremental_keys.extend(updates);
        self.incremental_keys.sort_unstable();
        self
    }

    /// Update mandatory metadata variables during Eapi registration.
    fn update_mandatory_keys(mut self, updates: &[MetadataKey]) -> Self {
        self.mandatory_keys.extend(updates);
        self.mandatory_keys.sort_unstable();
        self.metadata_keys.extend(updates);
        self.metadata_keys.sort_unstable();
        self
    }

    /// Update metadata variables during Eapi registration.
    fn update_metadata_keys(mut self, updates: &[MetadataKey]) -> Self {
        self.metadata_keys.extend(updates);
        self.metadata_keys.sort_unstable();
        self
    }

    /// Update econf options during Eapi registration.
    fn update_econf<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = EconfOption>,
    {
        for option in values {
            self.econf_options.replace(option);
        }
        self.econf_options.sort_unstable();
        self
    }

    /// Enable support for archive extensions during Eapi registration.
    fn enable_archives<'a, I>(mut self, extensions: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        self.archives.extend(extensions.into_iter().map(Into::into));
        self
    }

    /// Disable support for archive extensions during Eapi registration.
    fn disable_archives<'a, I>(mut self, extensions: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        for x in extensions {
            if !self.archives.remove(x) {
                unreachable!("EAPI {self}: disabling unknown archive format: {x}");
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
        self.env.sort_unstable();
        self
    }

    /// Disable support for build variables during Eapi registration.
    fn disable_env<I: IntoIterator<Item = Variable>>(mut self, variables: I) -> Self {
        for var in variables {
            if !self.env.swap_remove(&var) {
                unreachable!("EAPI {self}: disabling unregistered variable: {var}");
            }
        }
        self.env.sort_unstable();
        self
    }

    /// Update hooks during Eapi registration.
    fn update_hooks<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = HookBuilder>,
    {
        for builder in values {
            let phase = builder.phase;
            // TODO: replace with .entry() if/when upstream adds support
            let (_, phase) = self
                .phases
                .get_full_mut2(&phase)
                .unwrap_or_else(|| panic!("unregistered phase: {phase}"));
            let hooks = phase.hooks.entry(builder.kind).or_default();
            hooks.replace(builder.into());
            hooks.sort_unstable();
        }
        self
    }
}

/// EAPIs that have been dropped by pkgcraft.
static OLD_EAPIS: LazyLock<IndexSet<String>> = LazyLock::new(|| {
    ["0", "1", "2", "3", "4"]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
});

pub static EAPI5: LazyLock<Eapi> = LazyLock::new(|| {
    use crate::pkg::ebuild::MetadataKey::*;
    use crate::shell::commands::builtins::*;
    use crate::shell::environment::Variable::*;
    use crate::shell::hooks;
    use crate::shell::operations::OperationKind::*;
    use crate::shell::phase::{PhaseKind::*, eapi5};
    use crate::shell::scope::ScopeSet::*;
    use Feature::*;

    Eapi::new("5", None)
        .enable_features([DomoUsesDesttree, QueryHostRoot, TrailingSlash])
        .update_commands([
            adddeny.allowed_in([Phases]),
            addpredict.allowed_in([Phases]),
            addread.allowed_in([Phases]),
            addwrite.allowed_in([Phases]),
            assert.allowed_in([All]),
            best_version.allowed_in([Phases]).die(false),
            command_not_found_handle.allowed_in([All]),
            debug_print.allowed_in([All]),
            debug_print_function.allowed_in([All]),
            debug_print_section.allowed_in([All]),
            default.allowed_in([Phases]),
            default_pkg_nofetch.allowed_in([PkgNofetch]),
            default_src_compile.allowed_in([SrcCompile]),
            default_src_configure.allowed_in([SrcConfigure]),
            default_src_install.allowed_in([SrcInstall]),
            default_src_prepare.allowed_in([SrcPrepare]),
            default_src_test.allowed_in([SrcTest]),
            default_src_unpack.allowed_in([SrcUnpack]),
            die.allowed_in([All]),
            diropts.allowed_in([SrcInstall]),
            dobin.allowed_in([SrcInstall]),
            docinto.allowed_in([SrcInstall]),
            docompress.allowed_in([SrcInstall]),
            doconfd.allowed_in([SrcInstall]),
            dodir.allowed_in([SrcInstall]),
            dodoc.allowed_in([SrcInstall]),
            doenvd.allowed_in([SrcInstall]),
            doexe.allowed_in([SrcInstall]),
            doheader.allowed_in([SrcInstall]),
            dohtml.allowed_in([SrcInstall]),
            doinfo.allowed_in([SrcInstall]),
            doinitd.allowed_in([SrcInstall]),
            doins.allowed_in([SrcInstall]),
            dolib.allowed_in([SrcInstall]),
            dolib_a.allowed_in([SrcInstall]),
            dolib_so.allowed_in([SrcInstall]),
            doman.allowed_in([SrcInstall]),
            domo.allowed_in([SrcInstall]),
            dosbin.allowed_in([SrcInstall]),
            dosym.allowed_in([SrcInstall]),
            ebegin.allowed_in([Phases]),
            econf.allowed_in([SrcConfigure]),
            eend.allowed_in([Phases]).die(false),
            eerror.allowed_in([All]),
            einfo.allowed_in([All]),
            einfon.allowed_in([Phases]),
            einstall.allowed_in([SrcInstall]),
            elog.allowed_in([Phases]),
            emake.allowed_in([Phases]),
            ewarn.allowed_in([All]),
            exeinto.allowed_in([SrcInstall]),
            exeopts.allowed_in([SrcInstall]),
            export_functions.allowed_in([Eclass]),
            fowners.allowed_in([SrcInstall, PkgPreinst, PkgPostinst]),
            fperms.allowed_in([SrcInstall, PkgPreinst, PkgPostinst]),
            has.allowed_in([All]).die(false),
            has_version.allowed_in([Phases]).die(false),
            hasq.allowed_in([All]),
            hasv.allowed_in([All]).die(false),
            inherit.allowed_in([Global, Eclass]),
            insinto.allowed_in([SrcInstall]),
            insopts.allowed_in([SrcInstall]),
            into.allowed_in([SrcInstall]),
            keepdir.allowed_in([SrcInstall]),
            libopts.allowed_in([SrcInstall]),
            newbin.allowed_in([SrcInstall]),
            newconfd.allowed_in([SrcInstall]),
            newdoc.allowed_in([SrcInstall]),
            newenvd.allowed_in([SrcInstall]),
            newexe.allowed_in([SrcInstall]),
            newheader.allowed_in([SrcInstall]),
            newinitd.allowed_in([SrcInstall]),
            newins.allowed_in([SrcInstall]),
            newlib_a.allowed_in([SrcInstall]),
            newlib_so.allowed_in([SrcInstall]),
            newman.allowed_in([SrcInstall]),
            newsbin.allowed_in([SrcInstall]),
            nonfatal.allowed_in([All]).die(false),
            unpack.allowed_in([Phases]),
            use_.allowed_in([Phases]).die(false),
            use_enable.allowed_in([Phases]).die(false),
            use_with.allowed_in([Phases]).die(false),
            useq.allowed_in([Phases]).die(false),
            usev.allowed_in([Phases]).die(false),
            usex.allowed_in([Phases]).die(false),
            // phase stubs that force direct calls to error out
            pkg_config.allowed_in([All]).die(false),
            pkg_info.allowed_in([All]).die(false),
            pkg_nofetch.allowed_in([All]).die(false),
            pkg_postinst.allowed_in([All]).die(false),
            pkg_postrm.allowed_in([All]).die(false),
            pkg_preinst.allowed_in([All]).die(false),
            pkg_prerm.allowed_in([All]).die(false),
            pkg_pretend.allowed_in([All]).die(false),
            pkg_setup.allowed_in([All]).die(false),
            src_compile.allowed_in([All]).die(false),
            src_configure.allowed_in([All]).die(false),
            src_install.allowed_in([All]).die(false),
            src_prepare.allowed_in([All]).die(false),
            src_test.allowed_in([All]).die(false),
            src_unpack.allowed_in([All]).die(false),
        ])
        .update_phases([
            PkgConfig.into(),
            PkgInfo.into(),
            PkgNofetch.func(eapi5::pkg_nofetch),
            PkgPostinst.into(),
            PkgPostrm.into(),
            PkgPreinst.into(),
            PkgPrerm.into(),
            PkgPretend.into(),
            PkgSetup.into(),
            SrcCompile.dir(S).func(eapi5::src_compile),
            SrcConfigure.dir(S).func(eapi5::src_configure),
            SrcInstall.dir(S).func(eapi5::src_install),
            SrcPrepare.dir(S),
            SrcTest.dir(S).func(eapi5::src_test),
            SrcUnpack.dir(WORKDIR).func(eapi5::src_unpack),
        ])
        .update_operations([
            Build.phases([
                PkgSetup,
                SrcUnpack,
                SrcPrepare,
                SrcConfigure,
                SrcCompile,
                SrcTest,
                SrcInstall,
            ]),
            Install.phases([PkgPreinst, PkgPostinst]),
            Uninstall.phases([PkgPrerm, PkgPostrm]),
            Replace.phases([PkgPreinst, PkgPrerm, PkgPostrm, PkgPostinst]),
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
        .enable_archives([
            "tar", "gz", "Z", "tar.gz", "tgz", "tar.Z", "bz2", "bz", "tar.bz2", "tbz2",
            "tar.bz", "tbz", "zip", "ZIP", "jar", "7z", "7Z", "rar", "RAR", "LHA", "LHa",
            "lha", "lzh", "a", "deb", "lzma", "tar.lzma", "tar.xz", "xz",
        ])
        .update_env([
            A.allowed_in([Src, Phase(PkgNofetch)]),
            CATEGORY.allowed_in([All]),
            D.allowed_in([SrcInstall, PkgPreinst, PkgPostinst]).create(),
            DESTTREE.allowed_in([SrcInstall]),
            DISTDIR.allowed_in([Src, Global]),
            EBUILD_PHASE.allowed_in([Phases]),
            EBUILD_PHASE_FUNC.allowed_in([Phases]),
            ECLASSDIR.allowed_in([Src]),
            ED.allowed_in([SrcInstall, PkgPreinst, PkgPostinst]),
            EPREFIX.allowed_in([All]),
            EROOT.allowed_in([Pkg]),
            FILESDIR.allowed_in([Src, Global]),
            HOME.allowed_in([All]).create().external(),
            INSDESTTREE.allowed_in([SrcInstall]),
            MERGE_TYPE.allowed_in([Pkg]),
            P.allowed_in([All]),
            PF.allowed_in([All]),
            PN.allowed_in([All]),
            PORTDIR.allowed_in([Src]),
            PR.allowed_in([All]),
            PV.allowed_in([All]),
            PVR.allowed_in([All]),
            REPLACED_BY_VERSION.allowed_in([PkgPrerm, PkgPostrm]),
            REPLACING_VERSIONS.allowed_in([Pkg]),
            ROOT.allowed_in([Pkg]),
            S.allowed_in([Global, Src]).assignable(),
            T.allowed_in([All]).create(),
            TMPDIR.allowed_in([All]).external(),
            USE.allowed_in([All]),
            WORKDIR.allowed_in([Src, Global]).create(),
        ])
        // unexported, internal variables
        .update_env([DOCDESTTREE, EXEDESTTREE])
        .update_econf([
            EconfOption::new("--disable-dependency-tracking"),
            EconfOption::new("--disable-silent-rules"),
        ])
        .update_hooks([
            SrcInstall.pre("docompress", hooks::docompress::pre),
            SrcInstall.post("docompress", hooks::docompress::post),
        ])
});

pub static EAPI6: LazyLock<Eapi> = LazyLock::new(|| {
    use crate::shell::commands::builtins::*;
    use crate::shell::environment::Variable::*;
    use crate::shell::hooks;
    use crate::shell::phase::{PhaseKind::*, eapi6};
    use crate::shell::scope::ScopeSet::*;
    use Feature::*;

    Eapi::new("6", Some(&EAPI5))
        .enable_features([
            NonfatalDie,
            GlobalFailglob,
            UnpackExtendedPath,
            UnpackCaseInsensitive,
        ])
        .update_commands([
            eapply.allowed_in([SrcPrepare]),
            eapply_user.allowed_in([SrcPrepare]),
            einstalldocs.allowed_in([SrcInstall]),
            get_libdir.allowed_in([All]),
            in_iuse.allowed_in([Phases]).die(false),
        ])
        .disable_commands([einstall])
        .update_phases([
            SrcPrepare.dir(S).func(eapi6::src_prepare),
            SrcInstall.dir(S).func(eapi6::src_install),
        ])
        .update_econf([
            EconfOption::new("--docdir").value("${EPREFIX}/usr/share/doc/${PF}"),
            EconfOption::new("--htmldir").value("${EPREFIX}/usr/share/doc/${PF}/html"),
        ])
        .enable_archives(["txz"])
        .update_hooks([SrcPrepare.post("eapply_user", hooks::eapply_user::post)])
});

pub static EAPI7: LazyLock<Eapi> = LazyLock::new(|| {
    use crate::pkg::ebuild::MetadataKey::*;
    use crate::shell::commands::builtins::*;
    use crate::shell::environment::Variable::*;
    use crate::shell::hooks;
    use crate::shell::phase::PhaseKind::*;
    use crate::shell::scope::ScopeSet::*;
    use Feature::*;

    Eapi::new("7", Some(&EAPI6))
        .enable_features([QueryDeps])
        .disable_features([DomoUsesDesttree, QueryHostRoot, TrailingSlash])
        .update_commands([
            dostrip.allowed_in([SrcInstall]),
            eqawarn.allowed_in([All]),
            ver_cut.allowed_in([All]).die(false),
            ver_rs.allowed_in([All]).die(false),
            ver_test.allowed_in([All]).die(false),
        ])
        .disable_commands([dohtml, dolib, libopts])
        .update_dep_keys(&[BDEPEND])
        .update_incremental_keys(&[BDEPEND])
        .update_econf([EconfOption::new("--with-sysroot").value("${ESYSROOT:-/}")])
        .update_env([
            BROOT.allowed_in([
                Src,
                Phase(PkgSetup),
                Phase(PkgPreinst),
                Phase(PkgPostinst),
                Phase(PkgPrerm),
                Phase(PkgPostrm),
            ]),
            ESYSROOT.allowed_in([Src, Phase(PkgSetup)]),
            SYSROOT.allowed_in([Src, Phase(PkgSetup)]),
        ])
        // unexported, internal variables
        .update_env([DESTTREE, INSDESTTREE])
        // entirely removed
        .disable_env([PORTDIR, ECLASSDIR])
        .update_hooks([
            SrcInstall.pre("dostrip", hooks::dostrip::pre),
            SrcInstall.post("dostrip", hooks::dostrip::post),
        ])
});

pub static EAPI8: LazyLock<Eapi> = LazyLock::new(|| {
    use crate::pkg::ebuild::MetadataKey::*;
    use crate::shell::commands::builtins::*;
    use Feature::*;

    Eapi::new("8", Some(&EAPI7))
        .enable_features([ConsistentFileOpts, DosymRelative, SrcUriUnrestrict, UsevTwoArgs])
        .disable_commands([hasq, hasv, useq])
        .update_dep_keys(&[IDEPEND])
        .update_incremental_keys(&[IDEPEND, PROPERTIES, RESTRICT])
        .update_econf([
            EconfOption::new("--datarootdir").value("${EPREFIX}/usr/share"),
            EconfOption::new("--disable-static").markers(["--enable-static"]),
        ])
        .disable_archives(["7z", "7Z", "rar", "RAR", "LHA", "LHa", "lha", "lzh"])
});

pub static EAPI9: LazyLock<Eapi> = LazyLock::new(|| {
    use crate::shell::commands::builtins::*;
    use crate::shell::scope::ScopeSet::*;

    Eapi::new("9", Some(&EAPI8))
        .update_commands([
            edo.allowed_in([Phases]),
            pipestatus.allowed_in([All]),
            ver_replacing.allowed_in([Pkg]),
        ])
        .disable_commands([assert, domo])
});

/// Reference to the most recent, official EAPI.
pub static EAPI_LATEST_OFFICIAL: LazyLock<&'static Eapi> = LazyLock::new(|| &EAPI9);

/// The latest EAPI with extensions on top.
pub static EAPI_PKGCRAFT: LazyLock<Eapi> = LazyLock::new(|| {
    use Feature::*;
    Eapi::new("pkgcraft", Some(&EAPI_LATEST_OFFICIAL)).enable_features([RepoIds])
});

/// Reference to the most recent EAPI.
pub static EAPI_LATEST: LazyLock<&'static Eapi> = LazyLock::new(|| &EAPI_PKGCRAFT);

/// Ordered set of official, supported EAPIs.
pub static EAPIS_OFFICIAL: LazyLock<IndexSet<&'static Eapi>> = LazyLock::new(|| {
    [&*EAPI5, &*EAPI6, &*EAPI7, &*EAPI8, &*EAPI9]
        .into_iter()
        .collect()
});

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
    use camino_tempfile::{NamedUtf8TempFile, tempdir};

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
            assert_eq!(s, eapi.as_ref());
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
        let dir = tempdir().unwrap();
        let r: crate::Result<&Eapi> = dir.path().try_into();
        assert_err_re!(r, "failed reading EAPI: ");
        let file = NamedUtf8TempFile::new().unwrap();
        fs::write(&file, "8").unwrap();
        eapi = file.path().try_into().unwrap();
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
