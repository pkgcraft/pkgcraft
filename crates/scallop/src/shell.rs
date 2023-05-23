use std::ffi::{c_char, c_int, CStr, CString};
use std::{env, mem, process, ptr};

use nix::{
    sys::signal,
    unistd::{getpid, Pid},
};
use once_cell::sync::{Lazy, OnceCell};

use crate::builtins::ExecStatus;
use crate::{bash, Error};

use super::error;
use super::shm::create_shm;

/// Initialize the shell for library use.
pub fn init(restricted: bool) {
    let shm_name = format!("scallop-{}", getpid());
    let shm = create_shm(&shm_name, 4096).unwrap_or_else(|e| panic!("failed creating shm: {e}"));
    let name = CString::new("scallop").unwrap();
    unsafe {
        SHM.set(shm as *mut c_char)
            .expect("shm already initialized");
        bash::lib_error_handlers(Some(error::bash_error), Some(error::bash_warning_log));
        bash::lib_init(name.as_ptr() as *mut _, shm, restricted as i32);
    }

    // force main pid initialization
    Lazy::force(&PID);

    // shell name is saved since bash requires a valid pointer to it
    SHELL.set(name).expect("failed setting shell name");
}

/// Reset the shell back to a pristine state, optionally skipping a list of variables.
pub fn reset(ignore_vars: &[&str]) {
    let cached: Vec<(&str, String)> = ignore_vars
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

static PID: Lazy<Pid> = Lazy::new(getpid);
static SHELL: OnceCell<CString> = OnceCell::new();

/// Send a signal to the main bash process.
pub(crate) fn kill<T: Into<Option<signal::Signal>>>(signal: T) -> crate::Result<()> {
    signal::kill(*PID, signal.into()).map_err(|e| Error::Base(e.to_string()))
}

// Shared memory object for proxying subshell errors back to the main shell process.
static mut SHM: OnceCell<*mut c_char> = OnceCell::new();

/// Create an error message in shared memory.
pub(crate) fn set_shm_error(msg: &str) {
    let msg = CString::new(msg).unwrap();
    let mut data = msg.into_bytes_with_nul();

    // truncate error message if necessary
    if data.len() > 4096 {
        data = [&data[..4096], &[b'\0']].concat();
    }

    // write message into shared memory
    unsafe {
        let dst = *SHM.get().expect("uninitialized shell");
        ptr::copy_nonoverlapping(data.as_ptr(), dst as *mut u8, data.len());
    }
}

/// Raise an error from shared memory if one exists.
pub(crate) fn raise_shm_error() {
    unsafe {
        let dst = *SHM.get().expect("uninitialized shell");
        error::bash_error(dst);
        ptr::write_bytes(dst, b'\0', 4096);
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
    *PID == getpid()
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

#[cfg(test)]
mod tests {
    use crate::variables::*;

    use super::*;

    #[test]
    fn test_bash_version() {
        // TODO: add simple comparison check with version-compare if upstream merges set opts patch
        assert!(!BASH_VERSION.is_empty());
    }

    #[test]
    fn test_reset() {
        bind("VAR", "1", None, None).unwrap();
        assert_eq!(optional("VAR").unwrap(), "1");
        reset(&[]);
        assert_eq!(optional("VAR"), None);
    }
}
