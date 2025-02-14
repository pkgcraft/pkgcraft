use std::io::Write;

use itertools::Itertools;
use scallop::array::PipeStatus;
use scallop::ExecStatus;

use crate::io::stdout;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(name = "pipestatus", long_about = "Support PIPESTATUS assertions.")]
struct Command {
    #[arg(short = 'v')]
    verbose: bool,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;

    let pipestatus = PipeStatus::get();
    if cmd.verbose {
        writeln!(stdout(), "{}", pipestatus.iter().join(" "))?;
    }

    Ok(pipestatus.failed().into())
}

const USAGE: &str = "pipestatus";
make_builtin!("pipestatus", pipestatus_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_cmd, cmd_scope_tests, pipestatus};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(pipestatus, &[1]);
    }
}
