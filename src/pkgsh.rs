use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::path::Path;

use indexmap::IndexSet;
use itertools::Itertools;
use nix::unistd::isatty;
use scallop::builtins::{ExecStatus, ScopedOptions};
use scallop::variables::*;
use scallop::{functions, source, Shell, Error, Result};

use crate::eapi::{Eapi, Feature, Key};
use crate::pkgsh::builtins::Scope;

pub mod builtins;
mod install;
pub(crate) mod phase;
pub(crate) mod test;
pub(crate) mod unescape;
mod utils;

#[cfg(not(test))]
type StdinType = io::Stdin;
#[cfg(test)]
type StdinType = io::Cursor<Vec<u8>>;

struct Stdin {
    inner: StdinType,
}

impl Default for Stdin {
    fn default() -> Self {
        #[cfg(not(test))]
        let inner = io::stdin();
        #[cfg(test)]
        let inner = io::Cursor::new(vec![]);

        Stdin { inner }
    }
}

impl Stdin {
    fn get(&mut self) -> Result<&mut StdinType> {
        if !cfg!(test) && isatty(0).unwrap_or(false) {
            return Err(Error::Base("no input available, stdin is a tty".into()));
        }
        Ok(&mut self.inner)
    }
}

#[cfg(test)]
macro_rules! write_stdin {
    ($($arg:tt)*) => {
        crate::pkgsh::BUILD_DATA.with(|d| {
            write!(d.borrow_mut().stdin.inner, $($arg)*).unwrap();
            d.borrow_mut().stdin.inner.set_position(0);
        })
    }
}
#[cfg(test)]
use write_stdin;

struct Stdout {
    #[cfg(not(test))]
    inner: io::Stdout,
    #[cfg(test)]
    inner: io::Cursor<Vec<u8>>,
}

impl Default for Stdout {
    fn default() -> Self {
        #[cfg(not(test))]
        let inner = io::stdout();
        #[cfg(test)]
        let inner = io::Cursor::new(vec![]);

        Stdout { inner }
    }
}

macro_rules! write_stdout {
    ($($arg:tt)*) => {
        crate::pkgsh::BUILD_DATA.with(|d| write!(d.borrow_mut().stdout.inner, $($arg)*).unwrap())
    }
}
use write_stdout;

#[cfg(test)]
macro_rules! get_stdout {
    () => {
        crate::pkgsh::BUILD_DATA.with(|d| {
            let mut d = d.borrow_mut();
            let output = std::str::from_utf8(d.stdout.inner.get_ref()).unwrap();
            let output = String::from(output);
            d.stdout.inner = std::io::Cursor::new(vec![]);
            output
        })
    };
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
    #[cfg(not(test))]
    inner: io::Stderr,
    #[cfg(test)]
    inner: io::Cursor<Vec<u8>>,
}

impl Default for Stderr {
    fn default() -> Self {
        #[cfg(not(test))]
        let inner = io::stderr();
        #[cfg(test)]
        let inner = io::Cursor::new(vec![]);

        Stderr { inner }
    }
}

macro_rules! write_stderr {
    ($($arg:tt)*) => {
        crate::pkgsh::BUILD_DATA.with(|d| write!(d.borrow_mut().stderr.inner, $($arg)*).unwrap())
    }
}
use write_stderr;

#[cfg(test)]
macro_rules! get_stderr {
    () => {
        crate::pkgsh::BUILD_DATA.with(|d| {
            let mut d = d.borrow_mut();
            let output = std::str::from_utf8(d.stderr.inner.get_ref()).unwrap();
            let output = String::from(output);
            d.stderr.inner = std::io::Cursor::new(vec![]);
            output
        })
    };
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

#[derive(Default)]
pub struct BuildData {
    pub repo: String,
    pub eapi: &'static Eapi,

    stdin: Stdin,
    stdout: Stdout,
    stderr: Stderr,

    // mapping of variables conditionally exported to the build environment
    pub env: HashMap<String, String>,

    // TODO: proxy these fields via borrowed package reference
    pub distfiles: Vec<String>,
    pub user_patches: Vec<String>,

    pub phase: Option<phase::Phase>,
    pub user_patches_applied: bool,

    pub desttree: String,
    pub docdesttree: String,
    pub exedesttree: String,
    pub insdesttree: String,

    pub insopts: Vec<String>,
    pub diropts: Vec<String>,
    pub exeopts: Vec<String>,
    pub libopts: Vec<String>,

    // TODO: add default values listed in the spec
    pub compress_include: HashSet<String>,
    pub compress_exclude: HashSet<String>,
    pub strip_include: HashSet<String>,
    pub strip_exclude: HashSet<String>,

    pub iuse_effective: HashSet<String>,
    pub use_: HashSet<String>,

    /// Eclasses directly inherited by an ebuild.
    pub inherit: Vec<String>,
    /// Full set of eclasses inherited by an ebuild.
    pub inherited: IndexSet<String>,

    // ebuild metadata fields
    pub iuse: VecDeque<String>,
    pub required_use: VecDeque<String>,
    pub depend: VecDeque<String>,
    pub rdepend: VecDeque<String>,
    pub pdepend: VecDeque<String>,
    pub bdepend: VecDeque<String>,
    pub idepend: VecDeque<String>,
    pub properties: VecDeque<String>,
    pub restrict: VecDeque<String>,
}

impl BuildData {
    fn new() -> Self {
        let mut data = BuildData::default();
        // set build state defaults
        data.insopts.push("-m0644".into());
        data.libopts.push("-m0644".into());
        data.diropts.push("-m0755".into());
        data.exeopts.push("-m0755".into());
        data.desttree = "/usr".into();
        data
    }

    fn stdin(&mut self) -> Result<&mut StdinType> {
        self.stdin.get()
    }

    fn install(&self) -> install::Install {
        install::Install::new(self)
    }

    fn get_deque(&mut self, key: &Key) -> &mut VecDeque<String> {
        match key {
            Key::Iuse => &mut self.iuse,
            Key::RequiredUse => &mut self.required_use,
            Key::Depend => &mut self.depend,
            Key::Rdepend => &mut self.rdepend,
            Key::Pdepend => &mut self.pdepend,
            Key::Bdepend => &mut self.bdepend,
            Key::Idepend => &mut self.idepend,
            Key::Properties => &mut self.properties,
            Key::Restrict => &mut self.restrict,
            _ => panic!("unknown field name: {key}"),
        }
    }
}

thread_local! {
    pub static BUILD_DATA: RefCell<BuildData> = RefCell::new(BuildData::new());
}

pub struct PkgShell {}

impl PkgShell {
    pub fn new(data: BuildData) -> Self {
        // update thread local mutable for builtins
        BUILD_DATA.with(|d| d.replace(data));
        PkgShell {}
    }

    pub fn run_phase(&mut self, phase: &phase::Phase) -> Result<ExecStatus> {
        BUILD_DATA.with(|d| -> Result<ExecStatus> {
            d.borrow_mut().phase = Some(*phase);

            let eapi = d.borrow().eapi;
            let mut phase_name = ScopedVariable::new("EBUILD_PHASE");
            let mut phase_func_name = ScopedVariable::new("EBUILD_PHASE_FUNC");

            // enable phase builtins
            let _builtins = eapi.scoped_builtins(phase)?;

            phase_name.bind(phase.short_name(), None, None)?;
            if eapi.has(Feature::EbuildPhaseFunc) {
                phase_func_name.bind(phase, None, None)?;
            }

            // run user space pre-phase hooks
            if let Some(mut func) = functions::find(format!("pre_{phase}")) {
                func.execute(&[])?;
            }

            // run user space phase function, falling back to internal default
            match functions::find(phase) {
                Some(mut func) => func.execute(&[])?,
                None => match eapi.phases().get(phase) {
                    Some(phase) => phase.run()?,
                    None => return Err(Error::Base(format!("nonexistent phase: {phase}"))),
                },
            };

            // run user space post-phase hooks
            if let Some(mut func) = functions::find(format!("post_{phase}")) {
                func.execute(&[])?;
            }

            d.borrow_mut().phase = None;

            Ok(ExecStatus::Success)
        })
    }

    pub fn source_ebuild<P: AsRef<Path>>(&mut self, ebuild: P) -> Result<()> {
        let ebuild = ebuild.as_ref();
        if !ebuild.exists() {
            return Err(Error::Base(format!("nonexistent ebuild: {ebuild:?}")));
        }

        BUILD_DATA.with(|d| -> Result<()> {
            let eapi = d.borrow().eapi;
            let mut opts = ScopedOptions::default();

            // enable global builtins
            let _builtins = eapi.scoped_builtins(Scope::Global)?;

            if eapi.has(Feature::GlobalFailglob) {
                opts.enable(["failglob"])?;
            }

            source::file(&ebuild)?;

            // TODO: export default for $S

            // set RDEPEND=DEPEND if RDEPEND is unset
            if eapi.has(Feature::RdependDefault) && string_value("RDEPEND").is_none() {
                let depend = string_value("DEPEND").unwrap_or_else(|| String::from(""));
                bind("RDEPEND", &depend, None, None)?;
            }

            // prepend metadata keys that incrementally accumulate to eclass values
            let mut d = d.borrow_mut();
            for var in eapi.incremental_keys() {
                let deque = d.get_deque(var);
                if let Ok(data) = string_vec(var) {
                    // TODO: extend_left() should be implemented upstream for VecDeque
                    for item in data.into_iter().rev() {
                        deque.push_front(item);
                    }
                }
                // export the incrementally accumulated value
                bind(var, deque.iter().join(" "), None, None)?;
            }
            Ok(())
        })
    }

    pub fn reset(&mut self) {
        Shell::reset()
    }
}
