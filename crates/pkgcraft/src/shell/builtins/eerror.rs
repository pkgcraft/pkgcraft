use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::shell::{unescape::unescape_iter, write_stderr};

use super::{make_builtin, Scopes::Phases};

const LONG_DOC: &str = "Display error message.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let msg = unescape_iter(args)?.join(" ");
    write_stderr!("* {msg}\n")?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "eerror \"message\"";
make_builtin!("eerror", eerror_builtin, run, LONG_DOC, USAGE, &[("..", &[Phases])]);

#[cfg(test)]
mod tests {
    use crate::shell::assert_stderr;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as eerror;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(eerror, &[0]);
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
            eerror(&args).unwrap();
            assert_stderr!(expected);
        }
    }
}
