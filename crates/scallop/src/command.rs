use std::ffi::CString;
use std::ptr;
use std::str::FromStr;
use std::sync::LazyLock;

use bitflags::bitflags;
use itertools::Itertools;

use crate::error::{ok_or_error, Error};
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

pub struct Command {
    args: Vec<String>,
    flags: Option<Flags>,
}

impl Command {
    pub fn new<S: std::fmt::Display>(program: S) -> Self {
        Self {
            args: vec![program.to_string()],
            flags: None,
        }
    }

    pub fn args<I>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn subshell(&mut self, value: bool) -> &mut Self {
        if value {
            self.flags = Some(Flags::FORCE_SUBSHELL);
        }
        self
    }

    pub fn invert(&mut self, value: bool) -> &mut Self {
        if value {
            self.flags = Some(Flags::INVERT_RETURN);
        }
        self
    }

    pub fn execute(&self) -> crate::Result<ExecStatus> {
        let cmd: RawCommand = self.try_into()?;
        cmd.execute()
    }
}

impl FromStr for Command {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // TODO: use shlex to split string
        Ok(Self {
            args: s.split(' ').map(Into::into).collect(),
            flags: None,
        })
    }
}

impl TryFrom<&Command> for RawCommand {
    type Error = Error;

    fn try_from(value: &Command) -> crate::Result<Self> {
        let cmd: Self = value.args.iter().join(" ").parse()?;

        // apply flags
        if let Some(flags) = &value.flags {
            unsafe { (*cmd.ptr).flags |= flags.bits() as i32 };
        }

        Ok(cmd)
    }
}

#[derive(Debug)]
struct RawCommand {
    ptr: *mut bash::Command,
}

impl RawCommand {
    fn execute(&self) -> crate::Result<ExecStatus> {
        ok_or_error(|| match unsafe { bash::scallop_execute_command(self.ptr) } {
            0 => Ok(ExecStatus::Success),
            n => Err(Error::Status(ExecStatus::Failure(n))),
        })
    }
}

impl FromStr for RawCommand {
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

        Ok(Self { ptr: cmd })
    }
}

impl Drop for RawCommand {
    fn drop(&mut self) {
        unsafe { bash::dispose_command(self.ptr) };
    }
}

static COMMAND_MARKER: LazyLock<CString> =
    LazyLock::new(|| CString::new("Command::from_str").unwrap());

#[cfg(test)]
mod tests {
    use crate::variables::optional;

    use super::*;

    #[test]
    fn new_and_execute() {
        let cmd: Command = "VAR=0".parse().unwrap();
        cmd.execute().unwrap();
        assert_eq!(optional("VAR").unwrap(), "0");

        let mut cmd: Command = "VAR=1".parse().unwrap();
        cmd.subshell(true).execute().unwrap();
        assert_eq!(optional("VAR").unwrap(), "0");

        let mut cmd: Command = "VAR=1".parse().unwrap();
        assert!(cmd.invert(true).execute().is_err());
        assert_eq!(optional("VAR").unwrap(), "1");

        let cmd: Command = "exit 1".parse().unwrap();
        assert!(cmd.execute().is_err());
    }

    #[test]
    fn invalid() {
        assert!(RawCommand::from_str("|| {").is_err());
    }
}
