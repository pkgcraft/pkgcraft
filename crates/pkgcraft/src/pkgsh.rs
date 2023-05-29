use std::cell::UnsafeCell;
use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::{env, mem};

use indexmap::IndexSet;
use itertools::Itertools;
use nix::unistd::isatty;
use once_cell::sync::Lazy;
use scallop::builtins::{ExecStatus, ScopedOptions};
use scallop::variables::{self, *};
use scallop::Error;
use strum::{AsRefStr, Display};
use sys_info::os_release;

use crate::dep::Cpv;
use crate::eapi::{Eapi, Feature};
use crate::macros::build_from_paths;
use crate::pkg::Package;
use crate::pkgsh::builtins::{Scope, BUILTINS};
use crate::repo::{ebuild, Repository};
use crate::traits::SourceBash;
use crate::types::Deque;

pub mod builtins;
mod install;
pub(crate) mod metadata;
pub(crate) mod operations;
pub(crate) mod phase;
pub(crate) mod test;
mod unescape;
mod utils;

pub use metadata::Key;

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
        let build = crate::pkgsh::get_build_mut();
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
        let build = crate::pkgsh::get_build_mut();
        write!(build.stdout(), $($arg)*)?;
        build.stdout().flush()
    }}
}
use write_stdout;

#[cfg(test)]
macro_rules! get_stdout {
    () => {{
        let build = crate::pkgsh::get_build_mut();
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
        let output = crate::pkgsh::get_stdout!();
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
        let build = crate::pkgsh::get_build_mut();
        write!(build.stderr(), $($arg)*)?;
        build.stderr().flush()
    }}
}
use write_stderr;

#[cfg(test)]
macro_rules! get_stderr {
    () => {{
        let build = crate::pkgsh::get_build_mut();
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
        let output = crate::pkgsh::get_stderr!();
        assert_eq!(output, $expected);
    };
}
#[cfg(test)]
use assert_stderr;

#[derive(Debug)]
pub(crate) enum BuildState<'a> {
    Empty(&'static Eapi),
    Metadata(&'a crate::pkg::ebuild::RawPkg<'a>),
    Build(&'a crate::pkg::ebuild::Pkg<'a>),
}

impl Default for BuildState<'_> {
    fn default() -> Self {
        Self::Empty(Default::default())
    }
}

#[derive(Default)]
pub(crate) struct BuildData<'a> {
    state: BuildState<'a>,

    stdin: Stdin,
    stdout: Stdout,
    stderr: Stderr,

    // mapping of variables conditionally exported to the build environment
    env: HashMap<String, String>,

    // TODO: proxy these fields via borrowed package reference
    distfiles: Vec<String>,
    user_patches: Vec<String>,
    use_: HashSet<String>,

    scope: Scope,
    user_patches_applied: bool,

    desttree: String,
    docdesttree: String,
    exedesttree: String,
    insdesttree: String,

    insopts: Vec<String>,
    diropts: Vec<String>,
    exeopts: Vec<String>,
    libopts: Vec<String>,

    // TODO: add default values listed in the spec
    compress_include: HashSet<String>,
    compress_exclude: HashSet<String>,
    strip_include: HashSet<String>,
    strip_exclude: HashSet<String>,

    /// set of directly inherited eclasses
    inherit: IndexSet<String>,
    /// complete set of inherited eclasses
    inherited: IndexSet<String>,
    /// incremental metadata fields
    incrementals: HashMap<Key, Deque<String>>,
}

impl<'a> BuildData<'a> {
    fn new() -> Self {
        Self {
            insopts: vec!["-m0644".to_string()],
            libopts: vec!["-m0644".to_string()],
            diropts: vec!["-m0755".to_string()],
            exeopts: vec!["-m0755".to_string()],
            desttree: "/usr".into(),
            ..Default::default()
        }
    }

    #[cfg(test)]
    pub(crate) fn empty(eapi: &'static Eapi) {
        get_build_mut().state = BuildState::Empty(eapi);
    }

    pub(crate) fn from_raw_pkg(pkg: &'a crate::pkg::ebuild::RawPkg<'a>) {
        // TODO: remove this hack once BuildData is reworked
        let p = unsafe { mem::transmute(pkg) };
        let data = BuildData {
            state: BuildState::Metadata(p),
            ..BuildData::new()
        };
        update_build(data);
    }

    pub(crate) fn from_pkg(pkg: &'a crate::pkg::ebuild::Pkg<'a>) {
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
        }
    }

    /// Get the current CPV if it exists.
    fn cpv(&self) -> scallop::Result<&Cpv> {
        match &self.state {
            BuildState::Metadata(pkg) => Ok(pkg.cpv()),
            BuildState::Build(pkg) => Ok(pkg.cpv()),
            _ => Err(Error::Base(format!("cpv invalid for scope: {}", self.scope))),
        }
    }

    /// Get the current repo if it exists.
    fn repo(&self) -> scallop::Result<&ebuild::Repo> {
        match &self.state {
            BuildState::Metadata(pkg) => Ok(pkg.repo()),
            BuildState::Build(pkg) => Ok(pkg.repo()),
            _ => Err(Error::Base(format!("repo invalid for scope: {}", self.scope))),
        }
    }

    /// Get the current package being built if it exists.
    fn pkg(&self) -> scallop::Result<&crate::pkg::ebuild::Pkg> {
        match &self.state {
            BuildState::Build(pkg) => Ok(pkg),
            _ => Err(Error::Base(format!("pkg invalid for scope: {}", self.scope))),
        }
    }

    /// Get the current build phase if it exists.
    fn phase(&self) -> scallop::Result<phase::Phase> {
        match self.scope {
            Scope::Phase(k) => Ok(*self.eapi().phases().get(&k).expect("unknown scope phase")),
            scope => Err(Error::Base(format!("phase invalid for scope: {scope}"))),
        }
    }

    /// Get the value for a given build variable from the build state.
    fn get_var(&self, var: BuildVariable) -> scallop::Result<String> {
        use BuildVariable::*;
        match var {
            CATEGORY => self.cpv().map(|o| o.category().to_string()),
            P => self.cpv().map(|o| o.p()),
            PF => self.cpv().map(|o| o.pf()),
            PN => self.cpv().map(|o| o.package().to_string()),
            PR => self.cpv().map(|o| o.pr()),
            PV => self.cpv().map(|o| o.pv()),
            PVR => self.cpv().map(|o| o.pvr()),

            AA => self.pkg().map(|pkg| {
                pkg.src_uri()
                    .map(|d| d.iter_flatten().map(|u| u.filename()).join(" "))
                    .unwrap_or_default()
            }),
            FILESDIR => {
                let cpv = self.cpv()?;
                let path =
                    build_from_paths!(self.repo()?.path(), cpv.category(), cpv.package(), "files");
                Ok(path.into_string())
            }
            PORTDIR => self.repo().map(|r| r.path().to_string()),
            ECLASSDIR => self.repo().map(|r| r.path().join("eclass").into_string()),

            // TODO: alter based on config settings
            ROOT => Ok("".to_string()),
            EROOT => Ok("".to_string()),
            SYSROOT => Ok("".to_string()),
            ESYSROOT => Ok("".to_string()),
            BROOT => Ok("".to_string()),

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

            DESTTREE => Ok(self.desttree.to_string()),
            INSDESTTREE => Ok(self.insdesttree.to_string()),
            EBUILD_PHASE => self.phase().map(|p| p.short_name().to_string()),
            EBUILD_PHASE_FUNC => self.phase().map(|p| p.to_string()),
            KV => os_release().map_err(|e| Error::Base(format!("failed getting OS release: {e}"))),

            // TODO: alter for build vs install pkg state variants
            REPLACING_VERSIONS => Ok("".to_string()),
            REPLACED_BY_VERSION => Ok("".to_string()),

            // TODO: Implement the remaining variable values which will probably require reworking
            // BuildData into operation specific types since not all variables are exported in all
            // situations, e.g. source builds vs binary pkg merging.
            _ => Ok("TODO".to_string()),
        }
    }

    /// Cache and set build environment variables for the current EAPI and scope.
    fn set_vars(&mut self) -> scallop::Result<()> {
        for (var, scopes) in self.eapi().env() {
            if scopes.contains(&self.scope) {
                match self.env.get(var.as_ref()) {
                    Some(val) => {
                        bind(var, val, None, None)?;
                    }
                    None => {
                        let val = self.get_var(*var)?;
                        bind(var, &val, None, None)?;
                        self.env.insert(var.to_string(), val);
                    }
                }
            }
        }

        Ok(())
    }

    fn override_var(&self, var: BuildVariable, val: &str) -> scallop::Result<()> {
        if let Some(scopes) = self.eapi().env().get(&var) {
            if scopes.contains(&self.scope) {
                bind(var, val, None, None)?;
            } else {
                panic!("invalid scope {} for variable: {var}", self.scope);
            }
        }
        Ok(())
    }

    fn stdin(&mut self) -> scallop::Result<&mut dyn Read> {
        if !cfg!(test) && isatty(0).unwrap_or(false) {
            return Err(Error::Base("no input available, stdin is a tty".into()));
        }

        if !cfg!(test) {
            Ok(&mut self.stdin.inner)
        } else {
            Ok(&mut self.stdin.fake)
        }
    }

    fn stdout(&mut self) -> &mut dyn Write {
        if !cfg!(test) || scallop::shell::in_subshell() {
            &mut self.stdout.inner
        } else {
            &mut self.stdout.fake
        }
    }

    fn stderr(&mut self) -> &mut dyn Write {
        if !cfg!(test) || scallop::shell::in_subshell() {
            &mut self.stderr.inner
        } else {
            &mut self.stderr.fake
        }
    }

    fn destdir(&self) -> &str {
        self.env
            .get("ED")
            .unwrap_or_else(|| self.env.get("D").expect("undefined destdir vars: ED and D"))
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

        self.scope = Scope::Global;
        self.set_vars()?;

        let mut opts = ScopedOptions::default();
        if eapi.has(Feature::GlobalFailglob) {
            opts.enable(["failglob"])?;
        }

        // run global sourcing in restricted shell mode
        scallop::shell::restricted(|| value.source_bash())?;

        // set RDEPEND=DEPEND if RDEPEND is unset and DEPEND exists
        if eapi.has(Feature::RdependDefault) && variables::optional("RDEPEND").is_none() {
            if let Some(depend) = variables::optional("DEPEND") {
                bind("RDEPEND", depend, None, None)?;
            }
        }

        // prepend metadata keys that incrementally accumulate to eclass values
        if !self.inherited.is_empty() {
            for key in eapi.incremental_keys() {
                let deque = self.incrementals.entry(*key).or_insert_with(Deque::new);
                if let Some(data) = string_vec(key) {
                    deque.extend_left(data);
                }
                // export the incrementally accumulated value
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
    if cfg!(test) && !matches!(build.state, BuildState::Empty(_)) {
        scallop::shell::reset(&["PATH"]);
    }

    *build = state;
}

/// Initialize bash for library usage.
pub(crate) static BASH: Lazy<()> = Lazy::new(|| {
    unsafe { Lazy::force(&STATE) };
    scallop::shell::init(false);
    let builtins: Vec<_> = BUILTINS.iter().map(|&b| b.into()).collect();
    scallop::builtins::register(&builtins);
    // all builtins are enabled by default, access is restricted at runtime based on scope
    scallop::builtins::enable(&builtins).expect("failed enabling builtins");
});

#[derive(AsRefStr, Display, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum BuildVariable {
    // package specific
    CATEGORY,
    P,
    PF,
    PN,
    PR,
    PV,
    PVR,

    // environment specific
    A,
    AA,
    FILESDIR,
    DISTDIR,
    WORKDIR,
    S,
    PORTDIR,
    ECLASSDIR,
    ROOT,
    EROOT,
    SYSROOT,
    ESYSROOT,
    BROOT,
    T,
    TMPDIR,
    HOME,
    EPREFIX,
    D,
    ED,
    DESTTREE,
    INSDESTTREE,
    USE,
    EBUILD_PHASE,
    EBUILD_PHASE_FUNC,
    KV,
    MERGE_TYPE,
    REPLACING_VERSIONS,
    REPLACED_BY_VERSION,
}

#[cfg(test)]
mod tests {
    use scallop::variables;

    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::pkg::SourceablePackage;

    use super::*;

    #[test]
    fn global_scope_external_command() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // external commands are denied via restricted shell setting PATH=/dev/null
        let data = indoc::indoc! {r#"
            DESCRIPTION="unknown command failure"
            SLOT=0
            VAR=1
            ls /
            VAR=2
        "#};
        let pkg = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        BuildData::from_raw_pkg(&pkg);
        let r = pkg.source();
        assert_eq!(variables::optional("VAR").unwrap(), "1");
        assert_err_re!(r, "unknown command: ls");
    }

    #[test]
    fn global_scope_absolute_path_command() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // absolute command errors in restricted shells currently don't bail, so force them to
        scallop::builtins::set(["-e"]).unwrap();
        // absolute path for commands are denied via restricted shell
        let data = indoc::indoc! {r#"
            DESCRIPTION="unknown command failure"
            SLOT=0
            VAR=1
            /bin/ls /
            VAR=2
        "#};
        let pkg = t.create_ebuild_raw("cat/pkg-2", data).unwrap();
        BuildData::from_raw_pkg(&pkg);
        let r = pkg.source();
        assert_eq!(variables::optional("VAR").unwrap(), "1");
        assert_err_re!(r, ".+: /bin/ls: restricted: cannot specify `/' in command names: .+$");
    }
}
