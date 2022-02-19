use std::io::Write;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::super::unescape::unescape_string;
use super::{PkgBuiltin, PHASE};
use crate::pkgsh::write_stderr;

static LONG_DOC: &str = "Display informational messages without trailing newlines.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    for s in args {
        let unescaped = unescape_string(s)?;
        write_stderr!("{unescaped}");
    }

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
        &[("0-", &[PHASE])],
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
            (vec!["msg"], "msg"),
            (vec![r"\tmsg"], "\tmsg"),
            (vec![r"msg1\nmsg2"], "msg1\nmsg2"),
            (vec![r"msg1\\msg2"], "msg1\\msg2"),
        ] {
            einfon(&args).unwrap();
            assert_stderr!(expected);
        }
    }
}
