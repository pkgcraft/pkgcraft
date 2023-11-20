use std::ffi::{CStr, CString};
use std::ptr;
use std::str::FromStr;

use bitflags::bitflags;
use once_cell::sync::Lazy;

use crate::error::{ok_or_error, Error};
use crate::shell::track_top_level;
use crate::{bash, ExecStatus};

bitflags! {
    /// Flag values used with commands.
    pub struct Flags: u32 {
        const NONE = 0;
        const WANT_SUBSHELL = bash::CMD_WANT_SUBSHELL;
        const FORCE_SUBSHELL = bash::CMD_FORCE_SUBSHELL;
        const INVERT_RETURN = bash::CMD_INVERT_RETURN;
        const IGNORE_RETURN = bash::CMD_IGNORE_RETURN;
        const NO_FUNCTIONS = bash::CMD_NO_FUNCTIONS;
        const INHIBIT_EXPANSION = bash::CMD_INHIBIT_EXPANSION;
        const NO_FORK = bash::CMD_NO_FORK;
    }
}

#[derive(Debug)]
pub struct Command {
    ptr: *mut bash::Command,
}

impl Command {
    pub fn new<S: AsRef<str>>(s: S, flags: Option<Flags>) -> crate::Result<Self> {
        let cmd: Self = s.as_ref().parse()?;
        if let Some(flags) = flags {
            unsafe { (*cmd.ptr).flags |= flags.bits() as i32 };
        }
        Ok(cmd)
    }

    pub fn execute(&self) -> crate::Result<ExecStatus> {
        ok_or_error(|| {
            match track_top_level(|| unsafe { bash::scallop_execute_command(self.ptr) }) {
                0 => Ok(ExecStatus::Success),
                n => Err(Error::Status(ExecStatus::Failure(n))),
            }
        })
    }
}

impl Drop for Command {
    fn drop(&mut self) {
        unsafe { bash::dispose_command(self.ptr) };
    }
}

static COMMAND_MARKER: Lazy<CString> = Lazy::new(|| CString::new("Command::from_str").unwrap());

impl FromStr for Command {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cmd_str = CString::new(s).unwrap();
        let cmd_ptr = cmd_str.as_ptr() as *mut _;
        let name_ptr = COMMAND_MARKER.as_ptr();
        let cmd: *mut bash::Command;

        unsafe {
            // save input stream
            bash::push_stream(1);

            // parse command from string
            bash::with_input_from_string(cmd_ptr, name_ptr);
            cmd = match bash::parse_command() {
                0 => bash::copy_command(bash::GLOBAL_COMMAND),
                _ => return Err(Error::Base(format!("failed parsing: {s}"))),
            };

            // clean up global command
            bash::dispose_command(bash::GLOBAL_COMMAND);
            bash::GLOBAL_COMMAND = ptr::null_mut();

            // restore input stream
            bash::pop_stream();
        }

        Ok(Command { ptr: cmd })
    }
}

/// Get the currently running command name if one exists.
pub fn current<'a>() -> Option<&'a str> {
    unsafe {
        bash::CURRENT_COMMAND
            .as_ref()
            .map(|s| CStr::from_ptr(s).to_str().unwrap())
    }
}

/// Run a function under a named bash command scope.
pub(crate) fn cmd_scope<F>(name: &str, func: F) -> crate::Result<ExecStatus>
where
    F: FnOnce() -> crate::Result<ExecStatus>,
{
    let name = CString::new(name).unwrap();
    unsafe { bash::CURRENT_COMMAND = name.as_ptr() as *mut _ };
    let result = func();
    unsafe { bash::CURRENT_COMMAND = ptr::null_mut() };
    result
}

#[cfg(test)]
mod tests {
    use crate::variables::optional;

    use super::*;

    #[test]
    fn new_and_execute() {
        let cmd = Command::new("VAR=0", None).unwrap();
        cmd.execute().unwrap();
        assert_eq!(optional("VAR").unwrap(), "0");

        let cmd = Command::new("VAR=1", Some(Flags::WANT_SUBSHELL)).unwrap();
        cmd.execute().unwrap();
        assert_eq!(optional("VAR").unwrap(), "0");

        let cmd = Command::new("VAR=1", Some(Flags::FORCE_SUBSHELL)).unwrap();
        cmd.execute().unwrap();
        assert_eq!(optional("VAR").unwrap(), "0");

        let cmd = Command::new("VAR=1", Some(Flags::INVERT_RETURN)).unwrap();
        assert!(cmd.execute().is_err());
        assert_eq!(optional("VAR").unwrap(), "1");

        let cmd = Command::new("exit 1", None).unwrap();
        assert!(cmd.execute().is_err());
    }

    #[test]
    fn parse() {
        // invalid
        assert!(Command::from_str("|| {").is_err());

        // valid
        let s = "VAR=1";
        let cmd: Command = s.parse().unwrap();
        cmd.execute().unwrap();
        assert_eq!(optional("VAR").unwrap(), "1");
    }

    #[test]
    fn cmd_scope_and_current() {
        // no command running
        assert!(current().is_none());

        // fake a command
        cmd_scope("test", || {
            assert_eq!(current().unwrap(), "test");
            Ok(ExecStatus::Success)
        })
        .unwrap();
    }
}
