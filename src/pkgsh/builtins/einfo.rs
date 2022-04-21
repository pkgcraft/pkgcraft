use std::io::Write;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::super::unescape::unescape_string;
use super::{PkgBuiltin, PHASE};
use crate::pkgsh::write_stderr;

const LONG_DOC: &str = "Display informational message.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    write_stderr!("*");
    for s in args {
        let unescaped = unescape_string(s)?;
        write_stderr!(" {unescaped}");
    }
    write_stderr!("\n");

    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "einfo",
            func: run,
            help: LONG_DOC,
            usage: "einfo \"message\"",
        },
        &[("0-", &[PHASE])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as einfo;
    use crate::pkgsh::assert_stderr;

    #[test]
    fn invalid_args() {
        assert_invalid_args(einfo, &[0]);
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
            einfo(&args).unwrap();
            assert_stderr!(expected);
        }
    }
}
