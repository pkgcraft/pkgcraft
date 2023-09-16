use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::shell::{unescape::unescape_iter, write_stderr};

use super::{make_builtin, Scopes::Phases};

const LONG_DOC: &str = "Display informational message without trailing newline.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let msg = unescape_iter(args)?.join(" ");
    write_stderr!("* {msg}")?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "einfon \"message\"";
make_builtin!("einfon", einfon_builtin, run, LONG_DOC, USAGE, [("..", [Phases])]);

#[cfg(test)]
mod tests {
    use crate::shell::assert_stderr;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as einfon;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(einfon, &[0]);
    }

    #[test]
    fn output() {
        for (args, expected) in [
            (vec!["msg"], "* msg"),
            (vec![r"\tmsg"], "* \tmsg"),
            (vec!["msg1", "msg2"], "* msg1 msg2"),
            (vec![r"msg1\nmsg2"], "* msg1\nmsg2"),
            (vec![r"msg1\\msg2"], "* msg1\\msg2"),
        ] {
            einfon(&args).unwrap();
            assert_stderr!(expected);
        }
    }
}
