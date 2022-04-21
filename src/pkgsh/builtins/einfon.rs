use std::io::Write;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::super::unescape::unescape_vec;
use super::{PkgBuiltin, ALL};
use crate::pkgsh::write_stderr;

const LONG_DOC: &str = "Display informational message without trailing newline.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    let args = unescape_vec(args)?;
    let msg = args.join(" ");
    write_stderr!("* {msg}");

    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "einfon",
            func: run,
            help: LONG_DOC,
            usage: "einfon \"message\"",
        },
        &[("0-", &[ALL])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as einfon;
    use crate::pkgsh::assert_stderr;

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
