use std::borrow::Cow;
#[cfg(test)]
use std::cell::RefCell;
use std::process::Command;

use crate::Error;

fn run_cmd(cmd: &mut Command) -> crate::Result<()> {
    match cmd.status() {
        Ok(status) => {
            if status.success() {
                Ok(())
            } else {
                Err(Error::IO(format!("failed running: {cmd:?}")))
            }
        }
        Err(e) => Err(Error::IO(format!("failed running: {:?}: {e}", cmd.get_program()))),
    }
}

fn run_cmd_with_output(cmd: &mut Command) -> crate::Result<()> {
    match cmd.output() {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                let msg = String::from_utf8_lossy(&output.stderr);
                Err(Error::IO(format!("failed running: {}", msg.trim())))
            }
        }
        Err(e) => Err(Error::IO(format!("failed running: {:?}: {e}", cmd.get_program()))),
    }
}

/// Various command object functionality.
pub(crate) trait RunCommand {
    /// Run the command.
    fn run(&mut self) -> crate::Result<()>;
    /// Run the command while capturing output.
    fn run_with_output(&mut self) -> crate::Result<()>;
    /// Convert the command into a vector of its arguments.
    fn to_vec(&self) -> Vec<Cow<str>>;
}

impl RunCommand for Command {
    fn to_vec(&self) -> Vec<Cow<str>> {
        [self.get_program()]
            .into_iter()
            .chain(self.get_args())
            .map(|s| s.to_string_lossy())
            .collect()
    }

    #[cfg(not(test))]
    fn run(&mut self) -> crate::Result<()> {
        run_cmd(self)
    }

    #[cfg(not(test))]
    fn run_with_output(&mut self) -> crate::Result<()> {
        run_cmd_with_output(self)
    }

    #[cfg(test)]
    fn run(&mut self) -> crate::Result<()> {
        let cmd = self.to_vec().into_iter().map(String::from).collect();
        COMMANDS.with(|cmds| cmds.borrow_mut().push(cmd));

        RUN_COMMAND.with(|d| -> crate::Result<()> {
            if *d.borrow() { run_cmd(self) } else { Ok(()) }
        })
    }

    #[cfg(test)]
    fn run_with_output(&mut self) -> crate::Result<()> {
        let cmd = self.to_vec().into_iter().map(String::from).collect();
        COMMANDS.with(|cmds| cmds.borrow_mut().push(cmd));

        RUN_COMMAND.with(|d| -> crate::Result<()> {
            if *d.borrow() {
                run_cmd_with_output(self)
            } else {
                Ok(())
            }
        })
    }
}

#[cfg(test)]
thread_local! {
    static COMMANDS: RefCell<Vec<Vec<String>>> = RefCell::new(Default::default());
    static RUN_COMMAND: RefCell<bool> = const { RefCell::new(false) };
}

#[cfg(test)]
pub(crate) fn commands() -> Vec<Vec<String>> {
    COMMANDS.with(|cmds| cmds.take())
}

#[cfg(test)]
pub(crate) fn run_commands<F: FnOnce()>(func: F) {
    RUN_COMMAND.with(|d| {
        *d.borrow_mut() = true;
        func();
        *d.borrow_mut() = false;
    })
}
