use std::process::ExitCode;

mod command;
mod format;
mod subcmds;

fn main() -> anyhow::Result<ExitCode> {
    command::Command::run()
}
