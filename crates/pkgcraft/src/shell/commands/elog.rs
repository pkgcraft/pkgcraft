use std::io::Write;

use scallop::ExecStatus;

use crate::io::stderr;
use crate::shell::unescape::unescape;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "elog",
    disable_help_flag = true,
    long_about = "Display informational message of higher importance."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(required = false, allow_hyphen_values = true, default_value = "")]
    message: String,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let msg = unescape(&cmd.message)?;
    writeln!(stderr(), "* {msg}")?;
    // TODO: log these messages in some fashion
    Ok(ExecStatus::Success)
}

make_builtin!("elog", elog_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_cmd, cmd_scope_tests, functions::elog};
    use super::*;

    cmd_scope_tests!(r#"elog "a message""#);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(elog, &[2]);
    }

    #[test]
    fn output() {
        // no message
        elog(&[]).unwrap();
        assert_eq!(stderr().get(), "* \n");

        for (value, expected) in [
            ("", "* \n"),
            ("msg", "* msg\n"),
            (r"\tmsg", "* \tmsg\n"),
            ("msg1 msg2", "* msg1 msg2\n"),
            (r"msg1\nmsg2", "* msg1\nmsg2\n"),
            (r"msg1\\msg2", "* msg1\\msg2\n"),
        ] {
            elog(&[value]).unwrap();
            assert_eq!(stderr().get(), expected);
        }
    }
}
