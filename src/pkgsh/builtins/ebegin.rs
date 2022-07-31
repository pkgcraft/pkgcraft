use std::io::Write;

use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::pkgsh::write_stderr;

use super::super::unescape::unescape_vec;
use super::{make_builtin, ALL};

const LONG_DOC: &str = "Display information message when starting a process.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    let args = unescape_vec(args)?;
    let msg = args.join(" ");
    write_stderr!("* {msg} ...\n");

    Ok(ExecStatus::Success)
}

const USAGE: &str = "ebegin \"message\"";
make_builtin!("ebegin", ebegin_builtin, run, LONG_DOC, USAGE, &[("0-", &[ALL])]);

#[cfg(test)]
mod tests {
    use crate::pkgsh::assert_stderr;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as ebegin;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(ebegin, &[0]);
    }

    #[test]
    fn output() {
        for (args, expected) in [
            (vec!["msg"], "* msg ...\n"),
            (vec![r"\tmsg"], "* \tmsg ...\n"),
            (vec!["msg1", "msg2"], "* msg1 msg2 ...\n"),
            (vec![r"msg1\nmsg2"], "* msg1\nmsg2 ...\n"),
            (vec![r"msg1\\msg2"], "* msg1\\msg2 ...\n"),
        ] {
            ebegin(&args).unwrap();
            assert_stderr!(expected);
        }
    }
}
