use std::borrow::Borrow;
use std::cell::UnsafeCell;
use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::{env, mem};

use camino::Utf8Path;
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use nix::unistd::isatty;
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
use crate::repo::Repository;
use crate::restrict::Restrict;
use crate::test::TESTING;
use crate::traits::SourceBash;
use crate::types::Deque;

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

struct Stdin {
    inner: io::Stdin,
    fake: io::Cursor<Vec<u8>>,
}

impl Default for Stdin {
    fn default() -> Self {
        Self {
            inner: io::stdin(),
            fake: io::Cursor::new(vec![]),
        }
    }
}

#[cfg(test)]
macro_rules! write_stdin {
    ($($arg:tt)*) => {{
        let build = crate::shell::get_build_mut();
        write!(build.stdin.fake, $($arg)*).unwrap();
        build.stdin.fake.set_position(0);
    }}
}
#[cfg(test)]
use write_stdin;

struct Stdout {
    inner: io::Stdout,
    fake: io::Cursor<Vec<u8>>,
}

impl Default for Stdout {
    fn default() -> Self {
        Self {
            inner: io::stdout(),
            fake: io::Cursor::new(vec![]),
        }
    }
}

macro_rules! write_stdout {
    ($($arg:tt)*) => {{
        let build = crate::shell::get_build_mut();
        write!(build.stdout(), $($arg)*)?;
        build.stdout().flush()
    }}
}
use write_stdout;

#[cfg(test)]
macro_rules! get_stdout {
    () => {{
        let build = crate::shell::get_build_mut();
        let output = std::str::from_utf8(build.stdout.fake.get_ref()).unwrap();
        let output = String::from(output);
        build.stdout.fake = std::io::Cursor::new(vec![]);
        output
    }};
}
#[cfg(test)]
use get_stdout;

#[cfg(test)]
macro_rules! assert_stdout {
    ($expected:expr) => {
        let output = crate::shell::get_stdout!();
        assert_eq!(output, $expected);
    };
}
#[cfg(test)]
use assert_stdout;

struct Stderr {
    inner: io::Stderr,
    fake: io::Cursor<Vec<u8>>,
}

impl Default for Stderr {
    fn default() -> Self {
        Self {
            inner: io::stderr(),
            fake: io::Cursor::new(vec![]),
        }
    }
}

macro_rules! write_stderr {
    ($($arg:tt)*) => {{
        let build = crate::shell::get_build_mut();
        write!(build.stderr(), $($arg)*)?;
        build.stderr().flush()
    }}
}
use write_stderr;

#[cfg(test)]
macro_rules! get_stderr {
    () => {{
        let build = crate::shell::get_build_mut();
        let output = std::str::from_utf8(build.stderr.fake.get_ref()).unwrap();
        let output = String::from(output);
        build.stderr.fake = std::io::Cursor::new(vec![]);
        output
    }};
}
#[cfg(test)]
use get_stderr;

#[cfg(test)]
macro_rules! assert_stderr {
    ($expected:expr) => {
        let output = crate::shell::get_stderr!();
        assert_eq!(output, $expected);
    };
}
#[cfg(test)]
use assert_stderr;

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum BuildState<'a> {
    Empty(&'static Eapi),
    Metadata(&'a crate::pkg::ebuild::raw::Pkg<'a>),
    Build(&'a crate::pkg::ebuild::Pkg<'a>),
    Merge,
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
        get_build_mut().scope = self.0;
    }
}

#[derive(Default)]
pub(crate) struct BuildData<'a> {
    state: BuildState<'a>,

    /// nonfatal status set by the related builtin
    nonfatal: bool,

    stdin: Stdin,
    stdout: Stdout,
    stderr: Stderr,

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
    eclass_phases: IndexMap<phase::PhaseKind, &'a Eclass>,

    /// set of directly inherited eclasses
    inherit: IndexSet<&'a Eclass>,
    /// complete set of inherited eclasses
    inherited: IndexSet<&'a Eclass>,
    /// incremental metadata fields
    incrementals: HashMap<Key, Deque<String>>,
}

impl<'a> BuildData<'a> {
    fn new() -> Self {
        use Variable::*;
        let env = [(DESTTREE, "/usr"), (INSDESTTREE, ""), (DOCDESTTREE, ""), (EXEDESTTREE, "")]
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

    fn from_raw_pkg(pkg: &'a crate::pkg::ebuild::raw::Pkg<'a>) {
        // TODO: remove this hack once BuildData is reworked
        let p = unsafe { mem::transmute(pkg) };
        let data = BuildData {
            state: BuildState::Metadata(p),
            ..BuildData::new()
        };
        update_build(data);
    }

    fn from_pkg(pkg: &'a crate::pkg::ebuild::Pkg<'a>) {
        // TODO: remove this hack once BuildData is reworked
        let p = unsafe { mem::transmute(pkg) };
        let data = BuildData {
            state: BuildState::Build(p),
            ..BuildData::new()
        };
        update_build(data);
    }

    /// Get the current EAPI.
    fn eapi(&self) -> &'static Eapi {
        use BuildState::*;
        match &self.state {
            Empty(eapi) => eapi,
            Metadata(pkg) => pkg.eapi(),
            Build(pkg) => pkg.eapi(),
            Merge => todo!(),
        }
    }

    /// Get the current CPV if it exists.
    fn cpv(&'a self) -> scallop::Result<&'a Cpv<String>> {
        match &self.state {
            BuildState::Metadata(pkg) => Ok(pkg.cpv()),
            BuildState::Build(pkg) => Ok(pkg.cpv()),
            _ => Err(Error::Base(format!("cpv invalid for scope: {}", self.scope))),
        }
    }

    /// Get the current ebuild repo if it exists.
    fn ebuild_repo(&self) -> scallop::Result<&'a crate::repo::ebuild::Repo> {
        match &self.state {
            BuildState::Metadata(pkg) => Ok(pkg.repo()),
            BuildState::Build(pkg) => Ok(pkg.repo()),
            _ => Err(Error::Base(format!("ebuild repo invalid for scope: {}", self.scope))),
        }
    }

    /// Get the current repo if it exists.
    fn repo(&self) -> scallop::Result<Repo<'a>> {
        match &self.state {
            BuildState::Metadata(pkg) => Ok(Repo::Ebuild(pkg.repo())),
            BuildState::Build(pkg) => Ok(Repo::Ebuild(pkg.repo())),
            BuildState::Merge => Ok(Repo::Binary),
            _ => Err(Error::Base(format!("repo invalid for scope: {}", self.scope))),
        }
    }

    /// Get the current ebuild package being built if it exists.
    fn ebuild_pkg(&'a self) -> scallop::Result<Box<dyn EbuildPackage + 'a>> {
        match &self.state {
            BuildState::Build(pkg) => Ok(Box::new(pkg)),
            _ => Err(Error::Base(format!("ebuild pkg invalid for scope: {}", self.scope))),
        }
    }

    /// Get the current package being manipulated if it exists.
    fn pkg(&'a self) -> scallop::Result<Box<dyn Package + 'a>> {
        match &self.state {
            BuildState::Metadata(pkg) => Ok(Box::new(pkg)),
            BuildState::Build(pkg) => Ok(Box::new(pkg)),
            _ => Err(Error::Base(format!("pkg invalid for scope: {}", self.scope))),
        }
    }

    /// Change the current build to a temporary scope, reverting to the previous value when
    /// the returned value is dropped.
    fn scoped<T: Into<Scope>>(&mut self, value: T) -> Scoped {
        let scoped = Scoped(self.scope);
        self.scope = value.into();
        scoped
    }

    /// Get the current build phase if it exists.
    fn phase(&self) -> scallop::Result<&phase::Phase> {
        match &self.scope {
            Scope::Phase(k) => Ok(self.eapi().phases().get(k).expect("unknown scope phase")),
            scope => Err(Error::Base(format!("phase invalid for scope: {scope}"))),
        }
    }

    /// Get the current eclass if it exists.
    fn eclass(&self) -> scallop::Result<&'a Eclass> {
        match &self.scope {
            Scope::Eclass(Some(e)) => Ok(e),
            scope => Err(Error::Base(format!("eclass invalid for scope: {scope}"))),
        }
    }

    /// Get the cached value for a given build variable from the build state.
    fn env<V>(&self, var: V) -> scallop::Result<&str>
    where
        V: Borrow<Variable> + std::fmt::Display,
    {
        self.env
            .get(var.borrow())
            .map(|s| s.as_str())
            .ok_or_else(|| Error::Base(format!("{var} unset")))
    }

    /// Get the value for a given build variable from the build state.
    fn get_var(&self, var: Variable) -> scallop::Result<String> {
        use Variable::*;
        match var {
            CATEGORY => self.cpv().map(|o| o.category().to_string()),
            P => self.cpv().map(|o| o.p()),
            PF => self.cpv().map(|o| o.pf()),
            PN => self.cpv().map(|o| o.package().to_string()),
            PR => self.cpv().map(|o| o.pr()),
            PV => self.cpv().map(|o| o.pv()),
            PVR => self.cpv().map(|o| o.pvr()),

            FILESDIR => {
                let cpv = self.cpv()?;
                let path = build_path!(self.repo()?.path(), cpv.category(), cpv.package(), "files");
                Ok(path.to_string())
            }
            PORTDIR => self.repo().map(|r| r.path().to_string()),
            ECLASSDIR => self.repo().map(|r| r.path().join("eclass").to_string()),

            // TODO: alter based on config settings
            ROOT => Ok("".to_string()),
            EROOT => Ok("".to_string()),
            SYSROOT => Ok("".to_string()),
            ESYSROOT => Ok("".to_string()),
            BROOT => Ok("".to_string()),
            EPREFIX => Ok("".to_string()),

            // TODO: pull these values from the config
            T => {
                let path = std::env::temp_dir();
                let path = path
                    .to_str()
                    .ok_or_else(|| Error::Base(format!("non-unicode system tempdir: {path:?}")))?;
                Ok(path.to_string())
            }
            TMPDIR => self.get_var(T),
            HOME => self.get_var(T),
            WORKDIR => self.get_var(T),
            DISTDIR => self.get_var(T),
            ED => self.get_var(T),
            D => self.get_var(T),
            S => self.get_var(T),

            DESTTREE => Ok("/usr".to_string()),
            INSDESTTREE | DOCDESTTREE | EXEDESTTREE => Ok("".to_string()),

            EBUILD_PHASE => self.phase().map(|p| p.name().to_string()),
            EBUILD_PHASE_FUNC => self.phase().map(|p| p.to_string()),

            // TODO: alter for build vs install pkg state variants
            REPLACING_VERSIONS => Ok("".to_string()),
            REPLACED_BY_VERSION => Ok("".to_string()),
            MERGE_TYPE => Ok("source".to_string()),
            A => Ok("".to_string()),
            USE => Ok("".to_string()),
        }
    }

    /// Cache and set build environment variables for the current EAPI and scope.
    fn set_vars(&mut self) -> scallop::Result<()> {
        for var in self.eapi().env() {
            if var.exported(self.scope) {
                if let Some(val) = self.env.get(var.borrow()) {
                    var.bind(val)?;
                } else {
                    let val = self.get_var(var.into())?;
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
            if var.exported(self.scope) {
                var.bind(val)?;
            }
        }
        Ok(())
    }

    fn stdin(&mut self) -> scallop::Result<&mut dyn Read> {
        if !*TESTING && isatty(0).unwrap_or(false) {
            return Err(Error::Base("no input available, stdin is a tty".into()));
        }

        if !*TESTING {
            Ok(&mut self.stdin.inner)
        } else {
            Ok(&mut self.stdin.fake)
        }
    }

    fn stdout(&mut self) -> &mut dyn Write {
        if !*TESTING || scallop::shell::in_subshell() {
            &mut self.stdout.inner
        } else {
            &mut self.stdout.fake
        }
    }

    fn stderr(&mut self) -> &mut dyn Write {
        if !*TESTING || scallop::shell::in_subshell() {
            &mut self.stderr.inner
        } else {
            &mut self.stderr.fake
        }
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
        Lazy::force(&BASH);
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

static mut STATE: Lazy<UnsafeCell<BuildData<'static>>> =
    Lazy::new(|| UnsafeCell::new(BuildData::new()));

fn get_build_mut() -> &'static mut BuildData<'static> {
    unsafe { STATE.get_mut() }
}

fn update_build(state: BuildData<'static>) {
    let build = get_build_mut();

    // TODO: handle resets in external process pool
    if *TESTING && !matches!(build.state, BuildState::Empty(_)) {
        scallop::shell::reset(&["PATH"]);
    }

    *build = state;
}

type BuildFn = fn(build: &mut BuildData) -> scallop::Result<ExecStatus>;

/// Initialize bash for library usage.
pub(crate) static BASH: Lazy<()> = Lazy::new(|| {
    unsafe { Lazy::force(&STATE) };
    scallop::shell::init(false);
    // all builtins are enabled by default, access is restricted at runtime based on scope
    scallop::builtins::register(&*commands::BUILTINS);
    // restrict builtin loading and toggling
    scallop::builtins::disable(["enable"]).expect("failed disabling builtins");
});

/// Repo wrapper for commands supporting both source and binary operations.
#[derive(Debug)]
enum Repo<'a> {
    Ebuild(&'a crate::repo::ebuild::Repo),
    Binary,
}

impl Repo<'_> {
    fn path(&self) -> &Utf8Path {
        match self {
            Self::Ebuild(repo) => repo.path(),
            Self::Binary => todo!(),
        }
    }

    fn iter_cpv_restrict<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> impl Iterator<Item = Cpv<String>> + '_ {
        match self {
            Self::Ebuild(repo) => repo.iter_cpv_restrict(val),
            Self::Binary => todo!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use scallop::variables;

    use crate::config::Config;
    use crate::eapi::{EAPIS, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::pkg::{Build, Source};

    use super::*;

    #[test]
    fn global_scope_external_command() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

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
            let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", &data).unwrap();
            let r = raw_pkg.source();
            assert_eq!(variables::optional("VAR").unwrap(), "1");
            assert_err_re!(r, "unknown command: ls");
        }
    }

    #[test]
    fn global_scope_absolute_path_command() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

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
        let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-2", data).unwrap();
        let r = raw_pkg.source();
        assert_eq!(variables::optional("VAR").unwrap(), "1");
        assert_err_re!(r, ".+: /bin/ls: restricted: cannot specify `/' in command names$");
    }

    #[test]
    fn failglob() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="testing global failglob support"
                SLOT=0
                DOCS=( nonexistent* )
            "#};
            let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", &data).unwrap();
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
        let t = config.temp_repo("test", 0, None).unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            for cmd in eapi.commands().iter().filter(|b| !b.is_phase()) {
                let data = indoc::formatdoc! {r#"
                    EAPI={eapi}
                    DESCRIPTION="testing command override errors"
                    SLOT=0
                    {cmd}() {{ :; }}
                "#};
                let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", &data).unwrap();
                let r = raw_pkg.source();
                assert_err_re!(r, format!("EAPI {eapi} functionality overridden: {cmd}$"));
            }
        }
    }

    #[test]
    fn direct_phase_calls() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

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
                let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
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
                let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", &data).unwrap();
                let r = raw_pkg.source();
                assert_err_re!(r, format!("line 5: {phase}: error: direct phase call$"));
            }
        }
    }
}
