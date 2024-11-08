use std::borrow::Borrow;
use std::cell::UnsafeCell;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use std::{env, mem};

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use once_cell::sync::Lazy;
use scallop::builtins::{override_funcs, shopt};
use scallop::variables::*;
use scallop::{functions, Error, ExecStatus};

use crate::dep::Cpv;
use crate::eapi::{Eapi, Feature::GlobalFailglob};
use crate::macros::build_path;
use crate::pkg::ebuild::{metadata::Key, EbuildPackage};
use crate::pkg::{Package, RepoPackage};
use crate::repo::ebuild::Eclass;
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
pub mod scope;
pub(crate) mod test;
mod unescape;
mod utils;

use environment::Variable;
use scope::Scope;

#[derive(Debug)]
pub(crate) enum BuildState<'a> {
    Empty(&'static Eapi),
    Metadata(&'a crate::pkg::ebuild::raw::Pkg),
    Build(&'a crate::pkg::ebuild::Pkg),
}

impl Default for BuildState<'_> {
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
pub(crate) struct BuildData<'a> {
    state: BuildState<'a>,

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

impl BuildData<'_> {
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

    fn from_raw_pkg(pkg: &crate::pkg::ebuild::raw::Pkg) {
        // TODO: remove this hack once BuildData is reworked
        let p: &crate::pkg::ebuild::raw::Pkg = unsafe { mem::transmute(pkg) };
        let data = BuildData {
            state: BuildState::Metadata(p),
            ..BuildData::new()
        };
        update_build(data);
    }

    fn from_pkg(pkg: &crate::pkg::ebuild::Pkg) {
        // TODO: remove this hack once BuildData is reworked
        let p: &crate::pkg::ebuild::Pkg = unsafe { mem::transmute(pkg) };
        let data = BuildData {
            state: BuildState::Build(p),
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
        }
    }

    /// Get the current CPV if it exists.
    fn cpv(&self) -> &Cpv {
        match &self.state {
            BuildState::Metadata(pkg) => pkg.cpv(),
            BuildState::Build(pkg) => pkg.cpv(),
            _ => panic!("cpv invalid for scope: {}", self.scope),
        }
    }

    /// Get the current ebuild repo if it exists.
    fn ebuild_repo(&self) -> &crate::repo::ebuild::EbuildRepo {
        match &self.state {
            BuildState::Metadata(pkg) => pkg.repo(),
            BuildState::Build(pkg) => pkg.repo(),
            _ => panic!("ebuild repo invalid for scope: {}", self.scope),
        }
    }

    /// Get the current repo if it exists.
    fn repo(&self) -> Repo {
        match &self.state {
            BuildState::Metadata(pkg) => pkg.repo().clone().into(),
            BuildState::Build(pkg) => pkg.repo().clone().into(),
            _ => panic!("repo invalid for scope: {}", self.scope),
        }
    }

    /// Get the current ebuild package being built if it exists.
    fn ebuild_pkg(&self) -> Box<dyn EbuildPackage + '_> {
        match &self.state {
            BuildState::Build(pkg) => Box::new(pkg),
            _ => panic!("ebuild pkg invalid for scope: {}", self.scope),
        }
    }

    /// Get the current package being manipulated if it exists.
    fn pkg(&self) -> Box<dyn Package + '_> {
        match &self.state {
            BuildState::Metadata(pkg) => Box::new(pkg),
            BuildState::Build(pkg) => Box::new(pkg),
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
                let path = build_path!(self.repo().path(), cpv.category(), cpv.package(), "files");
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
            REPLACING_VERSIONS => Default::default(),
            REPLACED_BY_VERSION => Default::default(),
            MERGE_TYPE => "source".to_string(),
            A => Default::default(),
            USE => Default::default(),
        }
    }

    /// Cache and set build environment variables for the current EAPI and scope.
    fn set_vars(&mut self) -> scallop::Result<()> {
        for var in self.eapi().env() {
            if var.exported(&self.scope) {
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

    fn override_var(&mut self, var: Variable, val: &str) -> scallop::Result<()> {
        if let Some(var) = self.eapi().env().get(&var) {
            self.env.insert(var.into(), val.to_string());
            if var.exported(&self.scope) {
                var.bind(val)?;
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
        LazyLock::force(&BASH);
        let eapi = self.eapi();

        // remove external metadata vars from the environment
        for var in eapi.metadata_keys() {
            env::remove_var(var.as_ref());
        }

        // commands override functions
        override_funcs(eapi.commands(), true)?;
        // phase stubs override functions forcing direct calls to error out
        override_funcs(eapi.phases(), true)?;

        self.set_vars()?;

        if eapi.has(GlobalFailglob) {
            shopt::enable(&["failglob"])?;
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

// TODO: move to LazyLock once LazyLock::get_mut() is stabilized or state tracking is rewritten
static mut STATE: Lazy<UnsafeCell<BuildData<'static>>> =
    Lazy::new(|| UnsafeCell::new(BuildData::new()));

fn get_build_mut() -> &'static mut BuildData<'static> {
    unsafe { STATE.get_mut() }
}

fn update_build(state: BuildData<'static>) {
    let build = get_build_mut();

    // TODO: handle resets in external process pool
    if cfg!(test) && !matches!(build.state, BuildState::Empty(_)) {
        scallop::shell::reset(&["PATH"]);
    }

    *build = state;
}

type BuildFn = fn(build: &mut BuildData) -> scallop::Result<ExecStatus>;

/// Initialize bash for library usage.
pub(crate) static BASH: LazyLock<()> = LazyLock::new(|| {
    scallop::shell::init(false);
    // all builtins are enabled by default, access is restricted at runtime based on scope
    scallop::builtins::register(&*commands::BUILTINS);
    // restrict builtin loading and toggling
    scallop::builtins::disable(["enable"]).expect("failed disabling builtins");
});

#[cfg(test)]
mod tests {
    use scallop::variables;

    use crate::config::Config;
    use crate::eapi::{EAPIS, EAPIS_OFFICIAL};
    use crate::pkg::{Build, Source};
    use crate::test::assert_err_re;

    use super::*;

    #[test]
    fn global_scope_external_command() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();

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
            let raw_pkg = temp.create_raw_pkg_from_str("cat/pkg-1", &data).unwrap();
            let r = raw_pkg.source();
            assert_eq!(variables::optional("VAR").unwrap(), "1");
            assert_err_re!(r, "unknown command: ls");
        }
    }

    #[test]
    fn global_scope_absolute_path_command() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();

        // absolute command errors in restricted shells currently don't bail, so force them to
        scallop::builtins::set(["-e"]).unwrap();
        // absolute path for commands are denied via restricted shell
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="unknown command failure"
            SLOT=0
            VAR=1
            /bin/ls /
            VAR=2
        "#};
        let raw_pkg = temp.create_raw_pkg_from_str("cat/pkg-2", data).unwrap();
        let r = raw_pkg.source();
        assert_eq!(variables::optional("VAR").unwrap(), "1");
        assert_err_re!(r, ".+: /bin/ls: restricted: cannot specify `/' in command names$");
    }

    #[test]
    fn failglob() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="testing global failglob support"
                SLOT=0
                DOCS=( nonexistent* )
            "#};
            let raw_pkg = temp.create_raw_pkg_from_str("cat/pkg-1", &data).unwrap();
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
        let mut temp = config.temp_repo("test", 0, None).unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            for cmd in eapi.commands().iter().filter(|b| !b.is_phase()) {
                let data = indoc::formatdoc! {r#"
                    EAPI={eapi}
                    DESCRIPTION="testing command override errors"
                    SLOT=0
                    {cmd}() {{ :; }}
                "#};
                let raw_pkg = temp.create_raw_pkg_from_str("cat/pkg-1", &data).unwrap();
                let r = raw_pkg.source();
                assert_err_re!(r, format!("EAPI {eapi} functionality overridden: {cmd}$"));
            }
        }
    }

    #[test]
    fn direct_phase_calls() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();

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
                let pkg = temp.create_pkg_from_str("cat/pkg-1", &data).unwrap();
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
                let raw_pkg = temp.create_raw_pkg_from_str("cat/pkg-1", &data).unwrap();
                let r = raw_pkg.source();
                assert_err_re!(r, format!("line 5: {phase}: error: direct phase call$"));
            }
        }
    }
}
