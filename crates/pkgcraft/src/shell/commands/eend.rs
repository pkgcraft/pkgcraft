use std::io::Write;

use scallop::ExecStatus;

use crate::io::stderr;
use crate::shell::unescape::unescape;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "eend",
    long_about = "Indicates that the process begun with an ebegin message has completed."
)]
struct Command {
    status: i32,
    #[arg(required = false, default_value = "")]
    message: String,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    // TODO: support column-based formatting for success/failure indicators
    let mut stderr = stderr();
    if cmd.status == 0 {
        writeln!(stderr, "[ ok ]")?;
    } else {
        if !cmd.message.is_empty() {
            let msg = unescape(&cmd.message)?;
            write!(stderr, "{msg} ")?;
        }
        writeln!(stderr, "[ !! ]")?;
    }

    Ok(ExecStatus::from(cmd.status))
}

make_builtin!("eend", eend_builtin, false);

#[cfg(test)]
mod tests {
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, eend};
    use super::*;

    cmd_scope_tests!("eend $?");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(eend, &[0, 3]);
    }

    #[test]
    fn non_numeric_return_code() {
        assert!(eend(&["msg"]).is_err());
        assert!(eend(&["a", "msg"]).is_err());
    }

    #[test]
    fn output() {
        for (args, expected) in [
            (vec!["0"], "[ ok ]\n"),
            (vec!["0", "msg"], "[ ok ]\n"),
            (vec!["0", "msg1 msg2"], "[ ok ]\n"),
            (vec!["1"], "[ !! ]\n"),
            (vec!["1", "msg"], "msg [ !! ]\n"),
            (vec!["1", r"\tmsg"], "\tmsg [ !! ]\n"),
            (vec!["1", "msg1 msg2"], "msg1 msg2 [ !! ]\n"),
        ] {
            eend(&args).unwrap();
            assert_eq!(stderr().get(), expected);
        }
    }
}
