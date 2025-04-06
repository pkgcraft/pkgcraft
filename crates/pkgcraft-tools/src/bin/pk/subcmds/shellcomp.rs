use std::io;
use std::process::ExitCode;

use clap::{Args, CommandFactory};
use clap_complete::{generate, Shell};

use crate::command;

#[derive(Args)]
pub(crate) struct Command {
    shell: Shell,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let mut cmd = command::Command::command();
        generate(self.shell, &mut cmd, "pk", &mut io::stdout());
        Ok(ExitCode::SUCCESS)
    }
}
