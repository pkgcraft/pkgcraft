use std::io::Write;

use scallop::{Error, ExecStatus};

use crate::io::stderr;
use crate::shell::unescape::unescape_iter;

use super::make_builtin;

const LONG_DOC: &str = "Display informational message of higher importance.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let msg = unescape_iter(args)?.join(" ");
    writeln!(stderr(), "* {msg}")?;

    // TODO: log these messages in some fashion

    Ok(ExecStatus::Success)
}

const USAGE: &str = "elog \"message\"";
make_builtin!("elog", elog_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, cmd_scope_tests, elog};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(elog, &[0]);
    }

    #[test]
    fn output() {
        for (args, expected) in [
            (vec!["msg"], "* msg\n"),
            (vec![r"\tmsg"], "* \tmsg\n"),
            (vec!["msg1", "msg2"], "* msg1 msg2\n"),
            (vec![r"msg1\nmsg2"], "* msg1\nmsg2\n"),
            (vec![r"msg1\\msg2"], "* msg1\\msg2\n"),
        ] {
            elog(&args).unwrap();
            assert_eq!(stderr().get(), expected);
        }
    }
}
