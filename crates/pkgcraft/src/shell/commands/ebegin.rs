use std::io::Write;

use scallop::ExecStatus;

use crate::io::stderr;
use crate::shell::unescape::unescape;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "ebegin",
    disable_help_flag = true,
    long_about = "Display information message when starting a process."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(allow_hyphen_values = true)]
    message: String,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let msg = unescape(&cmd.message)?;
    writeln!(stderr(), "* {msg} ...")?;
    Ok(ExecStatus::Success)
}

make_builtin!("ebegin", ebegin_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_cmd, cmd_scope_tests, ebegin};
    use super::*;

    cmd_scope_tests!(r#"ebegin "a message""#);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(ebegin, &[0, 2]);
    }

    #[test]
    fn output() {
        for (value, expected) in [
            ("msg", "* msg ...\n"),
            (r"\tmsg", "* \tmsg ...\n"),
            ("msg1 msg2", "* msg1 msg2 ...\n"),
            (r"msg1\nmsg2", "* msg1\nmsg2 ...\n"),
            (r"msg1\\msg2", "* msg1\\msg2 ...\n"),
        ] {
            ebegin(&[value]).unwrap();
            assert_eq!(stderr().get(), expected);
        }
    }
}
