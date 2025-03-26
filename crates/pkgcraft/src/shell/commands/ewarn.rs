use std::io::Write;

use scallop::ExecStatus;

use crate::io::stderr;
use crate::shell::unescape::unescape;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(name = "ewarn", long_about = "Display warning message.")]
struct Command {
    #[arg(required = false, default_value = "")]
    message: String,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let msg = unescape(&cmd.message)?;
    writeln!(stderr(), "* {msg}")?;
    Ok(ExecStatus::Success)
}

make_builtin!("ewarn", ewarn_builtin, true);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_cmd, cmd_scope_tests, ewarn};
    use super::*;

    cmd_scope_tests!(r#"ewarn "a message""#);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(ewarn, &[2]);
    }

    #[test]
    fn output() {
        // no message
        ewarn(&[]).unwrap();
        assert_eq!(stderr().get(), "* \n");

        for (value, expected) in [
            ("", "* \n"),
            ("msg", "* msg\n"),
            (r"\tmsg", "* \tmsg\n"),
            ("msg1 msg2", "* msg1 msg2\n"),
            (r"msg1\nmsg2", "* msg1\nmsg2\n"),
            (r"msg1\\msg2", "* msg1\\msg2\n"),
        ] {
            ewarn(&[value]).unwrap();
            assert_eq!(stderr().get(), expected);
        }
    }
}
