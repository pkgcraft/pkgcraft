use std::ffi::{c_char, c_int, CStr, CString};
use std::sync::OnceLock;
use std::{env, mem, process, ptr};

use nix::unistd::{getpid, Pid};
use once_cell::sync::Lazy;

use crate::shm::create_shm;
use crate::{bash, error, ExecStatus};

// shell name
static SHELL: OnceLock<CString> = OnceLock::new();

/// Initialize the shell for library use.
pub fn init(restricted: bool) {
    let name = CString::new("scallop").unwrap();
    let shm = create_shm("scallop", 4096).unwrap_or_else(|e| panic!("failed creating shm: {e}"));
    unsafe {
        bash::lib_error_handlers(Some(error::bash_error), Some(error::bash_warning_log));
        bash::lib_init(name.as_ptr() as *mut _, shm, restricted as i32);
    }

    // shell name is saved since bash requires a valid pointer to it
    SHELL.set(name).expect("failed setting shell name");
}

/// Return the main shell process identifier.
fn pid() -> Pid {
    Pid::from_raw(unsafe { bash::SHELL_PID })
}

/// Reinitialize the shell when forking processes.
pub(crate) fn fork_init() {
    // use new shared memory object for proxying errors
    let shm = create_shm("scallop", 4096).unwrap_or_else(|e| panic!("failed creating shm: {e}"));
    unsafe {
        bash::SHM_BUF = shm;
        // update shell pid for child process
        bash::SHELL_PID = getpid().as_raw();
    }
}

/// Reset the shell back to a pristine state, optionally skipping a list of variables.
pub fn reset(ignore_vars: &[&str]) {
    let cached: Vec<_> = ignore_vars
        .iter()
        .filter_map(|&s| env::var(s).ok().map(|val| (s, val)))
        .collect();

    error::reset();
    unsafe { bash::lib_reset() };

    for (var, value) in cached {
        env::set_var(var, value);
    }
}

/// Start an interactive shell session.
pub fn interactive() {
    let mut argv_ptrs: Vec<_> = env::args()
        .map(|s| CString::new(s).unwrap().into_raw())
        .collect();
    let argc: c_int = argv_ptrs.len().try_into().unwrap();
    argv_ptrs.push(ptr::null_mut());
    argv_ptrs.shrink_to_fit();
    let argv = argv_ptrs.as_mut_ptr();
    mem::forget(argv_ptrs);

    let mut env_ptrs: Vec<_> = env::vars()
        .map(|(key, val)| CString::new(format!("{key}={val}")).unwrap().into_raw())
        .collect();
    env_ptrs.push(ptr::null_mut());
    env_ptrs.shrink_to_fit();
    let env = env_ptrs.as_mut_ptr();
    mem::forget(env_ptrs);

    let ret: i32;
    unsafe {
        bash::lib_error_handlers(Some(error::stderr_output), Some(error::stderr_output));
        ret = bash::bash_main(argc, argv, env);
    }
    process::exit(ret)
}

/// Create an error message in shared memory.
pub(crate) fn set_shm_error(msg: &str) {
    // convert unicode string into byte string
    let msg = CString::new(msg).unwrap();
    let mut data = msg.into_bytes_with_nul();

    // truncate error message as necessary
    if data.len() > 4096 {
        data = [&data[..4096], &[b'\0']].concat();
    }

    // write message into shared memory
    unsafe {
        let shm = bash::SHM_BUF as *mut u8;
        ptr::copy_nonoverlapping(data.as_ptr(), shm, data.len());
    }
}

/// Raise an error from shared memory if one exists.
pub(crate) fn raise_shm_error() {
    unsafe {
        let shm = bash::SHM_BUF as *mut c_char;
        error::bash_error(shm);
        ptr::write_bytes(shm, b'\0', 4096);
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
            env::set_var("PATH", s);
        }
    }

    result
}

/// Version string related to the bundled bash release.
pub static BASH_VERSION: Lazy<String> = Lazy::new(|| unsafe {
    let version = CStr::from_ptr(bash::DIST_VERSION).to_str().unwrap();
    format!("{version}.{}", bash::PATCH_LEVEL)
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
            Ok(ExecStatus::Success)
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
        reset(&[]);
        assert_eq!(variables::optional("VAR"), None);
    }

    #[test]
    fn test_reset_func() {
        assert!(functions::find("func").is_none());
        source::string("func() { :; }").unwrap();
        assert!(functions::find("func").is_some());
        reset(&[]);
        assert!(functions::find("func").is_none());
    }
}
