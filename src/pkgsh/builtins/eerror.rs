use std::io::Write;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::super::unescape::unescape_vec;
use super::{PkgBuiltin, ALL};
use crate::pkgsh::write_stderr;

const LONG_DOC: &str = "Display error message.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    let args = unescape_vec(args)?;
    let msg = args.join(" ");
    write_stderr!("* {msg}\n");

    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "eerror",
            func: run,
            help: LONG_DOC,
            usage: "eerror \"message\"",
        },
        &[("0-", &[ALL])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as eerror;
    use crate::pkgsh::assert_stderr;

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
