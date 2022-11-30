use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::{io, mem};

use camino::Utf8Path;
use indexmap::IndexSet;
use itertools::Itertools;
use nix::unistd::isatty;
use scallop::builtins::{ExecStatus, ScopedOptions};
use scallop::variables::{self, *};
use scallop::{functions, source, Error};
use strum::{AsRefStr, Display};
use sys_info::os_release;

use crate::atom::Atom;
use crate::eapi::{Eapi, Feature};
use crate::macros::{build_from_paths, extend_left};
use crate::metadata::Key;
use crate::pkgsh::builtins::Scope;
use crate::repo::{ebuild, Repository};

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
    fn get(&mut self) -> scallop::Result<&mut StdinType> {
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
pub(crate) struct BuildData<'a> {
    pub(crate) eapi: &'static Eapi,
    pub(crate) atom: Option<Atom>,
    pub(crate) repo: Option<&'a ebuild::Repo>,

    stdin: Stdin,
    stdout: Stdout,
    stderr: Stderr,

    // mapping of variables conditionally exported to the build environment
    pub(crate) env: HashMap<String, String>,

    // TODO: proxy these fields via borrowed package reference
    pub(crate) distfiles: Vec<String>,
    pub(crate) user_patches: Vec<String>,

    pub(crate) phase: Option<phase::Phase>,
    pub(crate) scope: Scope,
    pub(crate) user_patches_applied: bool,

    pub(crate) desttree: String,
    pub(crate) docdesttree: String,
    pub(crate) exedesttree: String,
    pub(crate) insdesttree: String,

    pub(crate) insopts: Vec<String>,
    pub(crate) diropts: Vec<String>,
    pub(crate) exeopts: Vec<String>,
    pub(crate) libopts: Vec<String>,

    // TODO: add default values listed in the spec
    pub(crate) compress_include: HashSet<String>,
    pub(crate) compress_exclude: HashSet<String>,
    pub(crate) strip_include: HashSet<String>,
    pub(crate) strip_exclude: HashSet<String>,

    pub(crate) iuse_effective: HashSet<String>,
    pub(crate) use_: HashSet<String>,

    /// Eclasses directly inherited by an ebuild.
    pub(crate) inherit: IndexSet<String>,
    /// Full set of eclasses inherited by an ebuild.
    pub(crate) inherited: IndexSet<String>,

    // ebuild metadata fields
    pub(crate) iuse: VecDeque<String>,
    pub(crate) required_use: VecDeque<String>,
    pub(crate) depend: VecDeque<String>,
    pub(crate) rdepend: VecDeque<String>,
    pub(crate) pdepend: VecDeque<String>,
    pub(crate) bdepend: VecDeque<String>,
    pub(crate) idepend: VecDeque<String>,
    pub(crate) properties: VecDeque<String>,
    pub(crate) restrict: VecDeque<String>,
}

impl BuildData<'_> {
    pub(crate) fn update(atom: &Atom, repo: &ebuild::Repo) {
        // TODO: remove this hack once BuildData is reworked
        // Drop the lifetime bound on the repo reference in order for it to be stored in BuildData
        // which currently requires `'static` due to its usage in a global, thread local, static
        // variable.
        let r = unsafe { mem::transmute(repo) };

        let data = BuildData {
            atom: Some(atom.clone()),
            repo: Some(r),
            insopts: vec!["-m0644".to_string()],
            libopts: vec!["-m0644".to_string()],
            diropts: vec!["-m0755".to_string()],
            exeopts: vec!["-m0755".to_string()],
            desttree: "/usr".into(),
            ..Default::default()
        };

        // set build state defaults
        BUILD_DATA.with(|d| d.replace(data));
    }

    fn set_vars(&mut self) -> scallop::Result<()> {
        for (var, scopes) in self.eapi.env() {
            if scopes.matches(self.scope) {
                if self.env.get(var.as_ref()).is_none() {
                    let val = var.get(self);
                    self.env.insert(var.to_string(), val);
                }
                bind(var, self.env.get(var.as_ref()).unwrap(), None, None)?;
            }
        }
        Ok(())
    }

    fn override_var(&self, var: BuildVariable, val: &str) -> scallop::Result<()> {
        if let Some(scopes) = self.eapi.env().get(&var) {
            if scopes.matches(self.scope) {
                bind(var, val, None, None)?;
            } else {
                panic!("invalid scope {:?} for variable: {var}", self.scope);
            }
        }
        Ok(())
    }

    fn stdin(&mut self) -> scallop::Result<&mut StdinType> {
        self.stdin.get()
    }

    fn destdir(&self) -> &str {
        self.env
            .get("ED")
            .unwrap_or_else(|| self.env.get("D").expect("undefined destdirs $ED and $D"))
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
    pub(crate) static BUILD_DATA: RefCell<BuildData<'static>> = RefCell::new(BuildData::default());
}

/// Initialize bash for library usage.
#[cfg(feature = "init")]
#[ctor::ctor]
fn initialize() {
    use crate::pkgsh::builtins::ALL_BUILTINS;
    scallop::shell::Shell::init();
    let builtins: Vec<_> = ALL_BUILTINS.values().map(|&b| b.into()).collect();
    scallop::builtins::register(&builtins);
    scallop::builtins::enable(&builtins).expect("failed enabling builtins");
}

// TODO: remove allow when public package building support is added
#[allow(dead_code)]
pub(crate) fn run_phase(phase: phase::Phase) -> scallop::Result<ExecStatus> {
    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        d.borrow_mut().phase = Some(phase);
        d.borrow_mut().scope = Scope::Phase(phase);
        d.borrow_mut().set_vars()?;

        // run user space pre-phase hooks
        if let Some(mut func) = functions::find(format!("pre_{phase}")) {
            func.execute(&[])?;
        }

        // run user space phase function, falling back to internal default
        match functions::find(phase) {
            Some(mut func) => func.execute(&[])?,
            None => match eapi.phases().get(&phase) {
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

pub(crate) fn source_ebuild(path: &Utf8Path) -> scallop::Result<()> {
    if !path.exists() {
        return Err(Error::Base(format!("nonexistent ebuild: {path:?}")));
    }

    BUILD_DATA.with(|d| -> scallop::Result<()> {
        let eapi = d.borrow().eapi;
        d.borrow_mut().scope = Scope::Global;
        d.borrow_mut().set_vars()?;

        let mut opts = ScopedOptions::default();
        if eapi.has(Feature::GlobalFailglob) {
            opts.enable(["failglob"])?;
        }

        source::file(path)?;

        // set RDEPEND=DEPEND if RDEPEND is unset and DEPEND exists
        if eapi.has(Feature::RdependDefault) && variables::optional("RDEPEND").is_none() {
            if let Some(depend) = variables::optional("DEPEND") {
                bind("RDEPEND", &depend, None, None)?;
            }
        }

        // prepend metadata keys that incrementally accumulate to eclass values
        if !d.borrow().inherited.is_empty() {
            let mut d = d.borrow_mut();
            for var in eapi.incremental_keys() {
                let deque = d.get_deque(var);
                if let Ok(data) = string_vec(var) {
                    extend_left!(deque, data.into_iter());
                }
                // export the incrementally accumulated value
                bind(var, deque.iter().join(" "), None, None)?;
            }
        }

        Ok(())
    })
}

#[derive(AsRefStr, Display, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum BuildVariable {
    P,
    PF,
    PN,
    CATEGORY,
    PV,
    PR,
    PVR,
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

impl BuildVariable {
    fn get(&self, build: &BuildData) -> String {
        use BuildVariable::*;
        let a = build.atom.as_ref().expect("missing required atom field");
        let v = a.version().expect("missing required versioned atom");
        match self {
            P => format!("{}-{}", a.package(), v.base()),
            PF => format!("{}-{}", a.package(), PVR.get(build)),
            PN => a.package().into(),
            CATEGORY => a.category().into(),
            PV => v.base().into(),
            PR => format!("r{}", v.revision()),
            PVR => match v.revision() == "0" {
                true => v.base().into(),
                false => v.into(),
            },
            FILESDIR => {
                let path = build_from_paths!(
                    build.repo.unwrap().path(),
                    a.category(),
                    a.package(),
                    "files"
                );
                path.into_string()
            }
            PORTDIR => build.repo.unwrap().path().to_string(),
            ECLASSDIR => build.repo.unwrap().path().join("eclass").into_string(),
            EBUILD_PHASE => build.phase.expect("missing phase").short_name().to_string(),
            EBUILD_PHASE_FUNC => build.phase.expect("missing phase").to_string(),
            KV => os_release().expect("failed to get OS version"),

            // TODO: Implement the remaining variable values which will probably require reworking
            // BuildData into operation specific types since not all variables are exported in all
            // situations, e.g. source builds vs binary pkg merging.
            _ => "TODO".to_string(),
        }
    }
}
