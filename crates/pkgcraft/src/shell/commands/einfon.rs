use std::io::Write;

use scallop::ExecStatus;

use crate::io::stderr;
use crate::shell::unescape::unescape;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "einfon",
    disable_help_flag = true,
    long_about = "Display informational message without trailing newline."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(required = false, allow_hyphen_values = true, default_value = "")]
    message: String,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let msg = unescape(&cmd.message)?;
    let mut stderr = stderr();
    write!(stderr, "* {msg}")?;
    stderr.flush()?;
    Ok(ExecStatus::Success)
}

make_builtin!("einfon", einfon_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_cmd, cmd_scope_tests, einfon};
    use super::*;

    cmd_scope_tests!(r#"einfon "a message""#);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(einfon, &[2]);
    }

    #[test]
    fn output() {
        // no message
        einfon(&[]).unwrap();
        assert_eq!(stderr().get(), "* ");

        for (value, expected) in [
            ("", "* "),
            ("msg", "* msg"),
            (r"\tmsg", "* \tmsg"),
            ("msg1 msg2", "* msg1 msg2"),
            (r"msg1\nmsg2", "* msg1\nmsg2"),
            (r"msg1\\msg2", "* msg1\\msg2"),
        ] {
            einfon(&[value]).unwrap();
            assert_eq!(stderr().get(), expected);
        }
    }
}
