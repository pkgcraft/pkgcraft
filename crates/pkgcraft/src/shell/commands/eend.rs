use scallop::{Error, ExecStatus};

use crate::shell::{unescape::unescape_iter, write_stderr};

use super::make_builtin;

const LONG_DOC: &str = "Display information message when starting a process.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let (status, args) = match args {
        [n, args @ ..] => match n.parse::<i32>() {
            Err(_) => return Err(Error::Base(format!("invalid return value: {n}"))),
            Ok(status) => (status, args),
        },
        [] => return Err(Error::Base("requires 1 or more args, got 0".to_string())),
    };

    // TODO: support column-based formatting for success/failure indicators
    if status == 0 {
        write_stderr!("[ ok ]\n")?;
    } else {
        if !args.is_empty() {
            let msg = unescape_iter(args)?.join(" ");
            write_stderr!("{msg} ")?;
        }
        write_stderr!("[ !! ]\n")?;
    }

    Ok(ExecStatus::from(status))
}

const USAGE: &str = "eend $?";
make_builtin!("eend", eend_builtin);

#[cfg(test)]
mod tests {
    use crate::macros::assert_err_re;
    use crate::shell::assert_stderr;

    use super::super::{assert_invalid_args, cmd_scope_tests, eend};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(eend, &[0]);
    }

    #[test]
    fn non_numeric_return_code() {
        assert_err_re!(eend(&["msg"]), "^invalid return value: msg$");
        assert_err_re!(eend(&["a", "msg"]), "^invalid return value: a$");
    }

    #[test]
    fn output() {
        for (args, expected) in [
            (vec!["0"], "[ ok ]\n"),
            (vec!["0", "msg"], "[ ok ]\n"),
            (vec!["0", "msg1", "msg2"], "[ ok ]\n"),
            (vec!["1"], "[ !! ]\n"),
            (vec!["1", "msg"], "msg [ !! ]\n"),
            (vec!["1", r"\tmsg"], "\tmsg [ !! ]\n"),
            (vec!["1", "msg1", "msg2"], "msg1 msg2 [ !! ]\n"),
        ] {
            eend(&args).unwrap();
            assert_stderr!(expected);
        }
    }
}
