use std::cell::UnsafeCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, Read, Write};
use std::mem;

use indexmap::IndexSet;
use itertools::Itertools;
use nix::unistd::isatty;
use once_cell::sync::Lazy;
use scallop::builtins::{ExecStatus, ScopedOptions};
use scallop::functions;
use scallop::variables::{self, *};
use strum::{AsRefStr, Display};
use sys_info::os_release;

use crate::dep::Cpv;
use crate::eapi::{Eapi, Feature};
use crate::macros::{build_from_paths, extend_left};
use crate::pkg::Package;
use crate::pkgsh::builtins::{Scope, ALL_BUILTINS};
use crate::repo::{ebuild, Repository};
use crate::traits::SourceBash;

pub mod builtins;
mod install;
pub(crate) mod metadata;
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
    Metadata(&'static Eapi, Cpv, &'a crate::repo::ebuild::Repo),
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

    captured_io: bool,
    stdin: Stdin,
    stdout: Stdout,
    stderr: Stderr,

    // mapping of variables conditionally exported to the build environment
    env: HashMap<String, String>,

    // TODO: proxy these fields via borrowed package reference
    distfiles: Vec<String>,
    user_patches: Vec<String>,
    use_: HashSet<String>,

    phase: Option<phase::Phase>,
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

    /// Eclasses directly inherited by an ebuild.
    inherit: IndexSet<String>,
    /// Full set of eclasses inherited by an ebuild.
    inherited: IndexSet<String>,

    // ebuild metadata fields
    iuse: VecDeque<String>,
    required_use: VecDeque<String>,
    depend: VecDeque<String>,
    rdepend: VecDeque<String>,
    pdepend: VecDeque<String>,
    bdepend: VecDeque<String>,
    idepend: VecDeque<String>,
    properties: VecDeque<String>,
    restrict: VecDeque<String>,
}

impl<'a> BuildData<'a> {
    fn new() -> Self {
        Self {
            captured_io: cfg!(test),
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

    pub(crate) fn update(cpv: &Cpv, repo: &'a ebuild::Repo, eapi: Option<&'static Eapi>) {
        // TODO: remove this hack once BuildData is reworked
        // Drop the lifetime bound on the repo reference in order for it to be stored in BuildData
        // which currently requires `'static` due to its usage in a global, thread local, static
        // variable.
        let r = unsafe { mem::transmute(repo) };

        let eapi = eapi.unwrap_or_default();
        let state = BuildState::Metadata(eapi, cpv.clone(), r);
        update_build(BuildData { state, ..BuildData::new() });
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

    fn eapi(&self) -> &'static Eapi {
        use BuildState::*;
        match &self.state {
            Empty(eapi) => eapi,
            Metadata(eapi, _, _) => eapi,
            Build(pkg) => pkg.eapi(),
        }
    }

    fn cpv(&self) -> &Cpv {
        use BuildState::*;
        match &self.state {
            Metadata(_, cpv, _) => cpv,
            Build(pkg) => pkg.cpv(),
            _ => panic!("cpv invalid for state: {:?}", self.state),
        }
    }

    fn repo(&self) -> &ebuild::Repo {
        use BuildState::*;
        match &self.state {
            Metadata(_, _, repo) => repo,
            Build(pkg) => pkg.repo(),
            _ => panic!("repo invalid for state: {:?}", self.state),
        }
    }

    fn pkg(&self) -> &crate::pkg::ebuild::Pkg {
        use BuildState::*;
        match &self.state {
            Build(pkg) => pkg,
            _ => panic!("pkg invalid for state: {:?}", self.state),
        }
    }

    fn set_vars(&mut self) -> scallop::Result<()> {
        for (var, scopes) in self.eapi().env() {
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
        if let Some(scopes) = self.eapi().env().get(&var) {
            if scopes.matches(self.scope) {
                bind(var, val, None, None)?;
            } else {
                panic!("invalid scope {:?} for variable: {var}", self.scope);
            }
        }
        Ok(())
    }

    fn stdin(&mut self) -> scallop::Result<&mut dyn Read> {
        if !cfg!(test) && isatty(0).unwrap_or(false) {
            return Err(scallop::Error::Base("no input available, stdin is a tty".into()));
        }

        match self.captured_io {
            false => Ok(&mut self.stdin.inner),
            true => Ok(&mut self.stdin.fake),
        }
    }

    fn stdout(&mut self) -> &mut dyn Write {
        match self.captured_io {
            false => &mut self.stdout.inner,
            true => &mut self.stdout.fake,
        }
    }

    fn stderr(&mut self) -> &mut dyn Write {
        match self.captured_io {
            false => &mut self.stderr.inner,
            true => &mut self.stderr.fake,
        }
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

struct State<'a>(UnsafeCell<BuildData<'a>>);

impl State<'_> {
    fn new() -> Self {
        Self(UnsafeCell::new(BuildData::new()))
    }
}

static mut STATE: Lazy<State<'static>> = Lazy::new(State::new);

fn get_build_mut() -> &'static mut BuildData<'static> {
    unsafe { STATE.0.get_mut() }
}

fn update_build(state: BuildData<'static>) {
    unsafe {
        STATE.0 = UnsafeCell::new(state);
    }
}

/// Initialize bash for library usage.
pub(crate) static BASH: Lazy<()> = Lazy::new(|| {
    unsafe { Lazy::force(&STATE) };
    scallop::shell::init(true);
    let builtins: Vec<_> = ALL_BUILTINS.values().map(|&b| b.into()).collect();
    scallop::builtins::register(&builtins);
    // all builtins are enabled by default, access is restricted at runtime based on scope
    scallop::builtins::enable(&builtins).expect("failed enabling builtins");
});

pub(crate) fn run_phase(phase: phase::Phase) -> scallop::Result<ExecStatus> {
    Lazy::force(&BASH);

    let build = get_build_mut();
    build.phase = Some(phase);
    build.scope = Scope::Phase(phase);
    build.set_vars()?;

    // run user space pre-phase hooks
    if let Some(mut func) = functions::find(format!("pre_{phase}")) {
        func.execute(&[])?;
    }

    // run user space phase function, falling back to internal default
    match functions::find(phase) {
        Some(mut func) => func.execute(&[])?,
        None => match build.eapi().phases().get(&phase) {
            Some(phase) => phase.run()?,
            None => return Err(scallop::Error::Base(format!("nonexistent phase: {phase}"))),
        },
    };

    // run user space post-phase hooks
    if let Some(mut func) = functions::find(format!("post_{phase}")) {
        func.execute(&[])?;
    }

    build.phase = None;

    Ok(ExecStatus::Success)
}

pub(crate) fn source_ebuild<T: SourceBash>(value: T) -> scallop::Result<ExecStatus> {
    Lazy::force(&BASH);

    let build = get_build_mut();
    build.scope = Scope::Global;
    build.set_vars()?;

    let mut opts = ScopedOptions::default();
    if build.eapi().has(Feature::GlobalFailglob) {
        opts.enable(["failglob"])?;
    }

    value.source_bash()?;

    // set RDEPEND=DEPEND if RDEPEND is unset and DEPEND exists
    if build.eapi().has(Feature::RdependDefault) && variables::optional("RDEPEND").is_none() {
        if let Some(depend) = variables::optional("DEPEND") {
            bind("RDEPEND", depend, None, None)?;
        }
    }

    // prepend metadata keys that incrementally accumulate to eclass values
    if !build.inherited.is_empty() {
        for var in build.eapi().incremental_keys() {
            let deque = build.get_deque(var);
            if let Ok(data) = string_vec(var) {
                extend_left!(deque, data.into_iter());
            }
            // export the incrementally accumulated value
            bind(var, deque.iter().join(" "), None, None)?;
        }
    }

    Ok(ExecStatus::Success)
}

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

impl BuildVariable {
    fn get(&self, build: &BuildData) -> String {
        use BuildVariable::*;
        match self {
            P => build.cpv().p(),
            PF => build.cpv().pf(),
            PN => build.cpv().package().to_string(),
            CATEGORY => build.cpv().category().to_string(),
            PV => build.cpv().pv(),
            PR => build.cpv().pr(),
            PVR => build.cpv().pvr(),
            FILESDIR => {
                let path = build_from_paths!(
                    build.repo().path(),
                    build.cpv().category(),
                    build.cpv().package(),
                    "files"
                );
                path.into_string()
            }
            PORTDIR => build.repo().path().to_string(),
            ECLASSDIR => build.repo().path().join("eclass").into_string(),
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

#[cfg(test)]
mod tests {
    use scallop::variables;

    use crate::config::Config;
    use crate::macros::assert_err_re;

    use super::*;

    #[test]
    fn source_ebuild_disables_external_cmds() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // external commands are denied via restricted shell setting PATH=/dev/null
        let data = indoc::indoc! {r#"
            DESCRIPTION="unknown command failure"
            SLOT=0
            VAR=1
            ls /
            VAR=2
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        BuildData::update(&cpv, &repo, None);
        let r = source_ebuild(&path);
        assert_eq!(variables::optional("VAR").unwrap(), "1");
        assert_err_re!(r, "unknown command: ls");

        // absolute command errors in restricted shells currently don't bail, so force them to
        scallop::builtins::set(&["-e"]).unwrap();
        // absolute path for commands are denied via restricted shell
        let data = indoc::indoc! {r#"
            DESCRIPTION="unknown command failure"
            SLOT=0
            VAR=1
            /bin/ls /
            VAR=2
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-2", data).unwrap();
        BuildData::update(&cpv, &repo, None);
        let r = source_ebuild(&path);
        assert_eq!(variables::optional("VAR").unwrap(), "1");
        assert_err_re!(r, ".+: /bin/ls: restricted: cannot specify `/' in command names$");
    }
}
