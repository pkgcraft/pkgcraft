use std::process::ExitCode;

mod command;
mod options;
mod subcmds;

fn main() -> anyhow::Result<ExitCode> {
    command::Command::run()
}
