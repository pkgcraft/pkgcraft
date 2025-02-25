use std::borrow::Borrow;
use std::cell::UnsafeCell;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use scallop::variables::*;
use scallop::{builtins, functions, Error, ExecStatus};

use crate::dep::Cpv;
use crate::eapi::{Eapi, Feature::GlobalFailglob};
use crate::macros::build_path;
use crate::pkg::ebuild::{metadata::Key, EbuildConfiguredPkg, EbuildPkg, EbuildRawPkg};
use crate::pkg::{Package, RepoPackage};
use crate::repo::ebuild::{EbuildRepo, Eclass};
use crate::repo::{Repo, Repository};
use crate::traits::SourceBash;
use crate::types::{Deque, OrderedSet};

pub mod commands;
pub mod environment;
pub(crate) mod hooks;
mod install;
mod metadata;
pub(crate) mod operations;
pub(crate) mod phase;
pub mod pool;
pub use pool::BuildPool;
pub mod scope;
pub(crate) mod test;
mod unescape;
mod utils;

use environment::Variable;
use scope::Scope;

// builtins that are permanently disabled
static DISABLED_BUILTINS: &[&str] = &["alias", "enable"];

// builtins that are disabled in global scope
static DISABLED_GLOBAL_BUILTINS: &[&str] =
    &["cd", "exit", "hash", "jobs", "kill", "pushd", "popd", "source", "."];

#[derive(Debug)]
pub(crate) enum BuildState {
    Empty(&'static Eapi),
    Metadata(EbuildRawPkg),
    Build(EbuildPkg),
    // TODO: use binpkgs for the replace state
    Replace { old: Vec<EbuildPkg>, new: EbuildPkg },
}

impl Default for BuildState {
    fn default() -> Self {
        Self::Empty(Default::default())
    }
}

#[derive(Debug)]
struct Scoped(Scope);

impl Drop for Scoped {
    fn drop(&mut self) {
        get_build_mut().scope = self.0.clone();
    }
}

#[derive(Default)]
pub(crate) struct BuildData {
    state: BuildState,

    /// nonfatal status set by the related builtin
    nonfatal: bool,

    // cache of variable values
    env: HashMap<Variable, String>,

    // TODO: proxy these fields via borrowed package reference
    distfiles: IndexSet<String>,
    user_patches: IndexSet<String>,
    use_: HashSet<String>,

    scope: Scope,
    user_patches_applied: bool,

    insopts: IndexSet<String>,
    diropts: IndexSet<String>,
    exeopts: IndexSet<String>,
    libopts: IndexSet<String>,

    compress_include: IndexSet<String>,
    compress_exclude: IndexSet<String>,
    strip_include: IndexSet<String>,
    strip_exclude: IndexSet<String>,

    /// phases defined by eclasses
    eclass_phases: IndexMap<phase::PhaseKind, Eclass>,

    /// set of directly inherited eclasses
    inherit: OrderedSet<Eclass>,
    /// complete set of inherited eclasses
    inherited: OrderedSet<Eclass>,
    /// incremental metadata fields
    incrementals: HashMap<Key, Deque<String>>,
}

impl BuildData {
    fn new() -> Self {
        let env = [
            (Variable::DESTTREE, "/usr"),
            (Variable::INSDESTTREE, ""),
            (Variable::DOCDESTTREE, ""),
            (Variable::EXEDESTTREE, ""),
        ]
        .into_iter()
        .map(|(v, s)| (v, s.to_string()))
        .collect();

        Self {
            insopts: ["-m0644".to_string()].into_iter().collect(),
            libopts: ["-m0644".to_string()].into_iter().collect(),
            diropts: ["-m0755".to_string()].into_iter().collect(),
            exeopts: ["-m0755".to_string()].into_iter().collect(),
            env,
            ..Default::default()
        }
    }

    #[cfg(test)]
    pub(crate) fn empty(eapi: &'static Eapi) {
        get_build_mut().state = BuildState::Empty(eapi);
    }

    fn from_raw_pkg(pkg: &EbuildRawPkg) {
        let data = BuildData {
            state: BuildState::Metadata(pkg.clone()),
            ..BuildData::new()
        };
        update_build(data);
    }

    fn from_pkg(pkg: &EbuildPkg) {
        let data = BuildData {
            state: BuildState::Build(pkg.clone()),
            ..BuildData::new()
        };
        update_build(data);
    }

    /// Get the current EAPI.
    fn eapi(&self) -> &'static Eapi {
        match &self.state {
            BuildState::Empty(eapi) => eapi,
            BuildState::Metadata(pkg) => pkg.eapi(),
            BuildState::Build(pkg) => pkg.eapi(),
            BuildState::Replace { new, .. } => new.eapi(),
        }
    }

    /// Get the current CPV if it exists.
    fn cpv(&self) -> &Cpv {
        match &self.state {
            BuildState::Metadata(pkg) => pkg.cpv(),
            BuildState::Build(pkg) => pkg.cpv(),
            BuildState::Replace { new, .. } => new.cpv(),
            _ => panic!("cpv invalid for scope: {}", self.scope),
        }
    }

    /// Get the current ebuild repo if it exists.
    fn ebuild_repo(&self) -> EbuildRepo {
        match &self.state {
            BuildState::Metadata(pkg) => pkg.repo(),
            BuildState::Build(pkg) => pkg.repo(),
            BuildState::Replace { new, .. } => new.repo(),
            _ => panic!("ebuild repo invalid for scope: {}", self.scope),
        }
    }

    /// Get the current repo if it exists.
    fn repo(&self) -> Repo {
        match &self.state {
            BuildState::Metadata(pkg) => pkg.repo().into(),
            BuildState::Build(pkg) => pkg.repo().into(),
            BuildState::Replace { new, .. } => new.repo().into(),
            _ => panic!("repo invalid for scope: {}", self.scope),
        }
    }

    /// Get the current ebuild package being built if it exists.
    fn ebuild_pkg(&self) -> EbuildPackage {
        match &self.state {
            BuildState::Build(pkg) => EbuildPackage::Pkg(pkg),
            BuildState::Replace { new, .. } => EbuildPackage::Pkg(new),
            _ => panic!("ebuild pkg invalid for scope: {}", self.scope),
        }
    }

    /// Get the current package being manipulated if it exists.
    fn pkg(&self) -> Box<dyn Package + '_> {
        match &self.state {
            BuildState::Metadata(pkg) => Box::new(pkg),
            BuildState::Build(pkg) => Box::new(pkg),
            BuildState::Replace { new, .. } => Box::new(new),
            _ => panic!("pkg invalid for scope: {}", self.scope),
        }
    }

    /// Change the current build to a temporary scope, reverting to the previous value when
    /// the returned value is dropped.
    fn scoped<T: Into<Scope>>(&mut self, value: T) -> Scoped {
        let scoped = Scoped(self.scope.clone());
        self.scope = value.into();
        scoped
    }

    /// Get the current build phase if it exists.
    fn phase(&self) -> &phase::Phase {
        match &self.scope {
            Scope::Phase(k) => self.eapi().phases().get(k).expect("unknown scope phase"),
            scope => panic!("phase invalid for scope: {scope}"),
        }
    }

    /// Get the current eclass if it exists.
    fn eclass(&self) -> Eclass {
        match &self.scope {
            Scope::Eclass(Some(eclass)) => eclass.clone(),
            scope => panic!("eclass invalid for scope: {scope}"),
        }
    }

    /// Get the cached value for a given build variable from the build state.
    fn env<V>(&self, var: V) -> &str
    where
        V: Borrow<Variable> + std::fmt::Display,
    {
        self.env
            .get(var.borrow())
            .unwrap_or_else(|| panic!("{var} unset"))
    }

    /// Get the value for a given build variable from the build state.
    fn get_var(&self, var: Variable) -> String {
        use Variable::*;
        match var {
            CATEGORY => self.cpv().category().to_string(),
            P => self.cpv().p(),
            PF => self.cpv().pf(),
            PN => self.cpv().package().to_string(),
            PR => self.cpv().pr(),
            PV => self.cpv().pv(),
            PVR => self.cpv().pvr(),

            FILESDIR => {
                let cpv = self.cpv();
                let path =
                    build_path!(self.repo().path(), cpv.category(), cpv.package(), "files");
                path.to_string()
            }
            PORTDIR => self.repo().path().to_string(),
            ECLASSDIR => self.repo().path().join("eclass").to_string(),

            // TODO: alter based on config settings
            ROOT => Default::default(),
            EROOT => Default::default(),
            SYSROOT => Default::default(),
            ESYSROOT => Default::default(),
            BROOT => Default::default(),
            EPREFIX => Default::default(),

            // TODO: pull these values from the config
            T => {
                let path = std::env::temp_dir();
                let path = path.to_str().expect("non-unicode system tempdir: {path:?}");
                path.to_string()
            }
            TMPDIR => self.get_var(T),
            HOME => self.get_var(T),
            WORKDIR => self.get_var(T),
            DISTDIR => self.get_var(T),
            ED => self.get_var(T),
            D => self.get_var(T),
            S => self.get_var(T),

            DESTTREE => "/usr".to_string(),
            INSDESTTREE | DOCDESTTREE | EXEDESTTREE => Default::default(),

            EBUILD_PHASE => self.phase().name().to_string(),
            EBUILD_PHASE_FUNC => self.phase().to_string(),

            // TODO: alter for build vs install pkg state variants
            REPLACING_VERSIONS => match &self.state {
                BuildState::Replace { old, .. } => old.iter().map(|p| p.cpv().pvr()).join(" "),
                _ => Default::default(),
            },
            REPLACED_BY_VERSION => match &self.state {
                BuildState::Replace { new, .. } => new.cpv().pvr(),
                _ => Default::default(),
            },
            MERGE_TYPE => match &self.state {
                BuildState::Build(_) => "source".into(),
                BuildState::Replace { .. } => "binary".into(),
                _ => Default::default(),
            },
            A => Default::default(),
            USE => Default::default(),
        }
    }

    /// Cache and set build environment variables for the current EAPI and scope.
    fn set_vars(&mut self) -> scallop::Result<()> {
        for var in self.eapi().env() {
            if var.is_exported(&self.scope) {
                if let Some(val) = self.env.get(var.borrow()) {
                    var.bind(val)?;
                } else {
                    let val = self.get_var(var.into());
                    var.bind(&val)?;
                    // cache static values when not generating metadata
                    if !matches!(self.state, BuildState::Metadata(_)) && var.is_static() {
                        self.env.insert(var.into(), val);
                    }
                }
            }
        }

        Ok(())
    }

    fn destdir(&self) -> &str {
        self.env.get(&Variable::ED).unwrap_or_else(|| {
            self.env
                .get(&Variable::D)
                .expect("undefined destdir vars: ED and D")
        })
    }

    fn install(&self) -> install::Install {
        install::Install::new(self)
    }

    fn source_ebuild<T: SourceBash>(&mut self, value: T) -> scallop::Result<ExecStatus> {
        let eapi = self.eapi();

        // explicitly disable builtins in global scope
        let _builtins = builtins::ScopedBuiltins::disable(DISABLED_GLOBAL_BUILTINS)?;

        // commands override functions
        builtins::override_funcs(eapi.commands(), true)?;
        // phase stubs override functions forcing direct calls to error out
        builtins::override_funcs(eapi.phases(), true)?;

        self.set_vars()?;

        if eapi.has(GlobalFailglob) {
            builtins::shopt::enable(&["failglob"])?;
        }

        // run global sourcing in restricted shell mode
        scallop::shell::restricted(|| value.source_bash())?;

        // create function aliases for eclass phases
        for (phase, eclass) in &self.eclass_phases {
            if functions::find(phase).is_none() {
                let func = format!("{eclass}_{phase}");
                if functions::find(&func).is_some() {
                    scallop::source::string(format!("{phase}() {{ {func} \"$@\"; }}"))?;
                } else {
                    return Err(Error::Base(format!(
                        "{eclass}.eclass: undefined phase function: {func}"
                    )));
                }
            }
        }

        // check for functions that override commands
        functions::visible()
            .into_iter()
            .filter(|s| {
                eapi.commands()
                    .get(s.as_str())
                    .map(|b| !b.is_phase())
                    .unwrap_or_default()
            })
            .try_for_each(|func| {
                Err(Error::Base(format!("EAPI {eapi} functionality overridden: {func}")))
            })?;

        // handle incremental metadata
        for key in eapi.incremental_keys() {
            let deque = self.incrementals.entry(*key).or_default();
            let export = !deque.is_empty();

            // prepend metadata keys that incrementally accumulate to any inherited values
            if let Some(data) = string_vec(key) {
                deque.extend_left(data);
            }

            // re-export the incrementally accumulated value if modified by inherits
            if export {
                bind(key, deque.iter().join(" "), None, None)?;
            }
        }

        Ok(ExecStatus::Success)
    }
}

static mut STATE: LazyLock<UnsafeCell<BuildData>> =
    LazyLock::new(|| UnsafeCell::new(BuildData::new()));

fn get_build_mut<'a>() -> &'a mut BuildData {
    unsafe { UnsafeCell::raw_get(&*STATE).as_mut().unwrap() }
}

fn update_build(state: BuildData) {
    let build = get_build_mut();

    // TODO: handle resets in external process pool
    if cfg!(test) && !matches!(build.state, BuildState::Empty(_)) {
        scallop::shell::reset(&["PATH"]);
    }

    *build = state;
}

type BuildFn = fn(build: &mut BuildData) -> scallop::Result<ExecStatus>;

/// Initialize bash for library usage.
pub(crate) fn init() -> scallop::Result<()> {
    scallop::shell::init();
    // all builtins are enabled by default, access is restricted at runtime based on scope
    builtins::register(&*commands::BUILTINS);
    // permanently disable builtins such as `enable` to restrict overriding builtins
    builtins::disable(DISABLED_BUILTINS)
}

/// Build wrapper for ebuild package variants.
enum EbuildPackage<'a> {
    Pkg(&'a EbuildPkg),
    Configured(&'a EbuildConfiguredPkg),
}

impl EbuildPackage<'_> {
    fn cpv(&self) -> &Cpv {
        match self {
            Self::Pkg(pkg) => pkg.cpv(),
            Self::Configured(pkg) => pkg.cpv(),
        }
    }

    fn iuse_effective(&self) -> &OrderedSet<String> {
        match self {
            Self::Pkg(pkg) => pkg.iuse_effective(),
            Self::Configured(pkg) => pkg.iuse_effective(),
        }
    }

    fn slot(&self) -> &str {
        match self {
            Self::Pkg(pkg) => pkg.slot(),
            Self::Configured(pkg) => pkg.slot(),
        }
    }
}

#[cfg(test)]
mod tests {
    use scallop::variables;

    use crate::config::Config;
    use crate::eapi::{EAPIS, EAPIS_OFFICIAL};
    use crate::pkg::{Build, Source};
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::test::assert_err_re;

    use super::*;

    #[test]
    fn global_scope_external_command() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            // external commands are denied via restricted shell setting PATH=/dev/null
            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="unknown command failure"
                SLOT=0
                VAR=1
                ls /
                VAR=2
            "#};
            temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
            let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
            let r = raw_pkg.source();
            assert_eq!(variables::optional("VAR").unwrap(), "1");
            assert_err_re!(r, "unknown command: ls");
        }
    }

    #[test]
    fn global_scope_absolute_path_command() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        // absolute command errors in restricted shells currently don't bail, so force them to
        builtins::set(["-e"]).unwrap();
        // absolute path for commands are denied via restricted shell
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="unknown command failure"
            SLOT=0
            VAR=1
            /bin/ls /
            VAR=2
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        let r = raw_pkg.source();
        assert_eq!(variables::optional("VAR").unwrap(), "1");
        assert_err_re!(r, ".+: /bin/ls: restricted: cannot specify `/' in command names$");
    }

    #[test]
    fn failglob() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="testing global failglob support"
                SLOT=0
                DOCS=( nonexistent* )
            "#};
            temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
            let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
            let r = raw_pkg.source();
            if eapi.has(GlobalFailglob) {
                assert_err_re!(r, "^line 4: no match: nonexistent\\*$");
            } else {
                assert!(r.is_ok());
            }
        }
    }

    #[test]
    fn cmd_overrides() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            for cmd in eapi.commands().iter().filter(|b| !b.is_phase()) {
                let data = indoc::formatdoc! {r#"
                    EAPI={eapi}
                    DESCRIPTION="testing command override errors"
                    SLOT=0
                    {cmd}() {{ :; }}
                "#};
                temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
                let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
                let r = raw_pkg.source();
                assert_err_re!(r, format!("EAPI {eapi} functionality overridden: {cmd}$"));
            }
        }
    }

    #[test]
    fn direct_phase_calls() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        for eapi in &*EAPIS {
            for phase in eapi.phases() {
                // phase scope
                let data = indoc::formatdoc! {r#"
                    EAPI={eapi}
                    DESCRIPTION="testing direct phase call errors in phase scope"
                    SLOT=0
                    {phase}() {{ :; }}
                    pkg_setup() {{ {phase}; }}
                "#};
                temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
                let pkg = repo.get_pkg("cat/pkg-1").unwrap();
                BuildData::from_pkg(&pkg);
                let r = pkg.build();
                assert_err_re!(r, format!("line 5: {phase}: error: direct phase call$"));

                // global scope
                let data = indoc::formatdoc! {r#"
                    EAPI={eapi}
                    DESCRIPTION="testing direct phase call errors in global scope"
                    SLOT=0
                    {phase}() {{ :; }}
                    {phase}
                "#};
                temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
                let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
                let r = raw_pkg.source();
                assert_err_re!(r, format!("line 5: {phase}: error: direct phase call$"));
            }
        }
    }
}
