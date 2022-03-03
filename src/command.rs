use std::borrow::Cow;
#[cfg(test)]
use std::cell::RefCell;
use std::process::Command;

use crate::{Error, Result};

fn run_cmd(cmd: &mut Command) -> Result<()> {
    match cmd.status() {
        Ok(r) => match r.success() {
            true => Ok(()),
            false => Err(Error::IO(format!("failed running: {cmd:?}"))),
        },
        Err(e) => Err(Error::IO(format!("failed running: {:?}: {e}", cmd.get_program()))),
    }
}

/// Various command object functionality.
pub(crate) trait RunCommand {
    /// Run the command.
    fn run(&mut self) -> Result<()>;
    /// Convert the command into a vector of its arguments.
    fn to_vec(&self) -> Vec<Cow<str>>;
}

impl RunCommand for Command {
    fn to_vec(&self) -> Vec<Cow<str>> {
        let mut args: Vec<Cow<str>> = vec![self.get_program().to_string_lossy()];
        args.extend(self.get_args().map(|s| s.to_string_lossy()));
        args
    }

    #[cfg(not(test))]
    fn run(&mut self) -> Result<()> {
        run_cmd(self)
    }

    #[cfg(test)]
    fn run(&mut self) -> Result<()> {
        let cmd = self.to_vec().into_iter().map(|s| String::from(s)).collect();
        COMMANDS.with(|cmds| cmds.borrow_mut().push(cmd));

        RUN_COMMAND.with(|d| -> Result<()> {
            match *d.borrow() {
                true => run_cmd(self),
                false => Ok(()),
            }
        })
    }
}

#[cfg(test)]
thread_local! {
    static COMMANDS: RefCell<Vec<Vec<String>>> = RefCell::new(Default::default());
    static RUN_COMMAND: RefCell<bool> = RefCell::new(false);
}

#[cfg(test)]
pub(crate) fn last_command() -> Option<Vec<String>> {
    COMMANDS.with(|cmds| cmds.borrow_mut().pop())
}

#[cfg(test)]
pub(crate) fn run_commands<F: FnOnce()>(func: F) {
    RUN_COMMAND.with(|d| {
        *d.borrow_mut() = true;
        func();
        *d.borrow_mut() = false;
    })
}
