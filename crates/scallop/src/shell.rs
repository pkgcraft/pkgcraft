use std::cmp::min;
use std::ffi::{CStr, CString, c_char, c_int};
use std::sync::LazyLock;
use std::{env, mem, ptr};

use indexmap::IndexMap;
use nix::sys::wait::{WaitStatus, waitpid};
use nix::unistd::{ForkResult, Pid, fork, getpid};

use crate::macros::{iter_to_array, str_to_raw};
use crate::shm::create_shm;
use crate::{ExecStatus, bash, error};

/// Wrapper to modify the shell environment on initialization.
#[derive(Default)]
pub struct Env {
    vars: IndexMap<String, String>,
}

impl Env {
    /// Create a new shell environment wrapper.
    pub fn new() -> Self {
        let mut env = Self::default();

        // pass through code coverage variables when testing
        if cfg!(any(feature = "test", test)) && env::var("CARGO_LLVM_COV").is_ok() {
            env = env.allow(["LLVM_PROFILE_FILE"]);
            env.extend(env::vars().filter(|(name, _)| name.starts_with("CARGO_LLVM_COV")));
        }

        env
    }

    /// Return an iterator of environment variables in "name=value" format.
    fn environ(&self) -> impl Iterator<Item = String> {
        self.vars
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
    }

    /// Allow environment variables to be passed through.
    pub fn allow<I, S>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: std::fmt::Display,
    {
        self.extend(vars.into_iter().map(|x| x.to_string()));
        self
    }
}

impl Extend<(String, String)> for Env {
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = (String, String)>,
    {
        self.vars.extend(iter);
    }
}

impl Extend<String> for Env {
    fn extend<I>(&mut self, value: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.vars.extend(
            value
                .into_iter()
                .filter_map(|name| env::var(&name).ok().map(|value| (name, value))),
        );
    }
}

/// Initialize shared memory for proxying errors.
fn shm_init() {
    let shm =
        create_shm("scallop", 4096).unwrap_or_else(|e| panic!("failed creating shm: {e}"));
    unsafe {
        bash::SHM_BUF = shm;
    }
}

/// Initialize the shell for library use.
pub fn init(env: Env) {
    shm_init();
    let mut env = iter_to_array!(env.environ(), str_to_raw);
    unsafe {
        bash::lib_error_handlers(Some(bash_error), Some(error::bash_warning_log));
        bash::lib_init(env.as_mut_ptr());
    }
}

/// Bash callback to convert bash errors into native errors.
#[unsafe(no_mangle)]
extern "C" fn bash_error(msg: *mut c_char) {
    error::bash_error(msg, 1)
}

/// Return the main shell process identifier.
fn pid() -> Pid {
    Pid::from_raw(unsafe { bash::SHELL_PID })
}

/// Reinitialize the shell inside a forked process.
pub fn fork_init() {
    // use separate shared memory for proxying errors
    shm_init();

    unsafe {
        // update shell pid for child process
        bash::SHELL_PID = getpid().as_raw();
    }
}

/// Reset the shell back to a pristine state, optionally setting the shell environment.
pub fn reset(env: Env) {
    error::reset();
    let mut env = iter_to_array!(env.environ(), str_to_raw);
    unsafe { bash::lib_reset(env.as_mut_ptr()) };
}

pub struct Interactive {
    args: Vec<String>,
    env: Env,
}

impl Default for Interactive {
    fn default() -> Self {
        Self {
            args: vec![String::from("scallop")],
            env: Default::default(),
        }
    }
}

impl Interactive {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn args<I>(mut self, args: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn env<I, S1, S2>(mut self, env: I) -> Self
    where
        I: IntoIterator<Item = (S1, S2)>,
        S1: Into<String>,
        S2: Into<String>,
    {
        self.env
            .extend(env.into_iter().map(|(s1, s2)| (s1.into(), s2.into())));
        self
    }

    fn convert_args(self) -> (*mut *mut c_char, i32, *mut *mut c_char) {
        let mut argv_ptrs = iter_to_array!(self.args.into_iter(), str_to_raw);
        let argc: c_int = argv_ptrs.len().try_into().unwrap();
        let argv = argv_ptrs.as_mut_ptr();
        mem::forget(argv_ptrs);

        let mut env_ptrs = iter_to_array!(self.env.environ(), str_to_raw);
        let env = env_ptrs.as_mut_ptr();
        mem::forget(env_ptrs);

        (argv, argc - 1, env)
    }

    /// Run an interactive shell.
    pub fn run(self) {
        let (argv, argc, env) = self.convert_args();
        unsafe {
            bash::lib_error_handlers(Some(error::stderr_output), Some(error::stderr_output));
            bash::bash_main(argc, argv, env);
        }
    }

    /// Run an interactive shell in a forked process, returning the exit status.
    pub fn fork(self) -> ExecStatus {
        let mut ret: i32 = -1;
        unsafe {
            bash::lib_error_handlers(Some(error::stderr_output), Some(error::stderr_output));
            match fork() {
                Ok(ForkResult::Parent { child }) => {
                    if let Ok(WaitStatus::Exited(_, status)) = waitpid(child, None) {
                        ret = status;
                    }
                }
                Ok(ForkResult::Child) => {
                    let (argv, argc, env) = self.convert_args();
                    bash::bash_main(argc, argv, env);
                    unreachable!("child shell didn't exit");
                }
                _ => unreachable!("failed forking shell"),
            }
        }

        ExecStatus::from(ret)
    }
}

/// Create an error message in shared memory.
pub(crate) fn set_shm_error(msg: &str, bail: bool) {
    // convert unicode string into byte string
    let data = CString::new(msg).unwrap().into_bytes_with_nul();
    let len = min(data.len(), 4096);

    // determine error status
    let status = if bail { bash::EX_LONGJMP as u8 } else { 1 };

    // write to shared memory
    unsafe {
        let shm = bash::SHM_BUF as *mut u8;
        // write message into shared memory
        ptr::copy_nonoverlapping(data.as_ptr(), shm, len);
        // truncate message
        ptr::write_bytes(shm.offset(4094), 0, 1);
        // write status indicator
        ptr::write_bytes(shm.offset(4095), status, 1);
    }
}

/// Raise an error from shared memory if one exists.
pub(crate) fn raise_shm_error() {
    unsafe {
        let shm = bash::SHM_BUF as *mut u8;
        if *shm != 0 {
            let msg = bash::SHM_BUF as *mut c_char;
            let status = *shm.offset(4095);
            error::bash_error(msg, status);
            // reset message
            ptr::write_bytes(shm, 0, 1);
        }
    }
}

/// Toggle restricted shell mode.
pub fn toggle_restricted(status: bool) {
    unsafe { bash::scallop_toggle_restricted(status as i32) }
}

/// Returns true if currently operating in a subshell, false otherwise.
pub fn in_subshell() -> bool {
    subshell_level() > 0
}

/// Returns the count of nested subshells (also available via $BASH_SUBSHELL).
pub fn subshell_level() -> i32 {
    unsafe { bash::SUBSHELL_LEVEL }
}

/// Returns true if currently operating in the main process.
pub fn in_main() -> bool {
    pid() == getpid()
}

/// Returns true if currently operating in restricted mode.
pub fn is_restricted() -> bool {
    unsafe { bash::RESTRICTED != 0 }
}

/// Returns true if shell started in restricted mode.
pub fn is_restricted_shell() -> bool {
    unsafe { bash::RESTRICTED_SHELL != 0 }
}

/// Run a function in restricted mode.
pub fn restricted<F>(func: F) -> crate::Result<ExecStatus>
where
    F: FnOnce() -> crate::Result<ExecStatus>,
{
    let orig_path = env::var("PATH").ok();
    let orig_restricted = is_restricted();

    if !orig_restricted {
        toggle_restricted(true);
    }

    let result = func();

    if !orig_restricted {
        toggle_restricted(false);

        // restore the original PATH
        if let Some(s) = orig_path {
            unsafe { env::set_var("PATH", s) };
        }
    }

    result
}

/// Version string related to the bundled bash release.
pub static BASH_VERSION: LazyLock<String> = LazyLock::new(|| unsafe {
    let version = CStr::from_ptr(bash::DIST_VERSION).to_str().unwrap();
    let patch = bash::PATCH_LEVEL;
    format!("{version}.{patch}")
});

/// Returns the line number of the currently executing command.
pub fn executing_line_number() -> i32 {
    unsafe { bash::executing_line_number() }
}

#[cfg(test)]
mod tests {
    use crate::{functions, source, variables};

    use super::*;

    #[test]
    fn test_restricted() {
        // shell isn't started in restricted mode
        assert!(!is_restricted_shell());
        assert!(!is_restricted());

        // enable restricted shell
        toggle_restricted(true);
        assert!(is_restricted());

        // disable restricted shell
        toggle_restricted(false);
        assert!(!is_restricted());

        // use restricted scope
        restricted(|| {
            assert!(is_restricted());
            // nested calls work as expected
            restricted(|| {
                assert!(is_restricted());
                Ok(ExecStatus::Success)
            })
        })
        .unwrap();
        assert!(!is_restricted());
    }

    #[test]
    fn test_bash_version() {
        // TODO: add simple comparison check with version-compare if upstream merges set opts patch
        assert!(!BASH_VERSION.is_empty());
    }

    #[test]
    fn test_reset_var() {
        variables::bind("VAR", "1", None, None).unwrap();
        assert_eq!(variables::optional("VAR").unwrap(), "1");
        reset(Env::default());
        assert_eq!(variables::optional("VAR"), None);
    }

    #[test]
    fn test_reset_func() {
        assert!(functions::find("func").is_none());
        source::string("func() { :; }").unwrap();
        assert!(functions::find("func").is_some());
        reset(Env::default());
        assert!(functions::find("func").is_none());
    }

    #[test]
    fn test_interactive() {
        // forked success
        let status = Interactive::new()
            .args(["-c", "exit 0"])
            .env([("PATH", "/dev/null")])
            .fork();
        assert_eq!(status, ExecStatus::Success);

        // forked failure
        let status = Interactive::new()
            .args(["-c", "cd nonexistent || exit 10"])
            .fork();
        assert_eq!(status, ExecStatus::Failure(10));

        // direct process exit
        Interactive::new().args(["-c", "exit 0"]).run()
    }
}
