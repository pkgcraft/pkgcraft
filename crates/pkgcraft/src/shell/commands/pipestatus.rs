use std::io::Write;

use itertools::Itertools;
use scallop::ExecStatus;
use scallop::array::PipeStatus;

use crate::io::stdout;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(name = "pipestatus", long_about = "Support PIPESTATUS assertions.")]
struct Command {
    #[arg(short = 'v')]
    verbose: bool,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;

    let pipestatus = PipeStatus::get();
    if cmd.verbose {
        writeln!(stdout(), "{}", pipestatus.iter().join(" "))?;
    }

    Ok(pipestatus.failed().into())
}

make_builtin!("pipestatus", pipestatus_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_cmd, cmd_scope_tests, functions::pipestatus};

    cmd_scope_tests!("pipestatus");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(pipestatus, &[1]);
    }
}
