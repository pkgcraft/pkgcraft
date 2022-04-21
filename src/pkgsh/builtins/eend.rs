use std::io::Write;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::super::unescape::unescape_vec;
use super::{PkgBuiltin, ALL};
use crate::pkgsh::write_stderr;

const LONG_DOC: &str = "Display information message when starting a process.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    let ret = args[0];
    let ret = ret
        .parse::<i32>()
        .map_err(|_| Error::Builtin(format!("invalid return value: {ret}")))?;
    let ret = ExecStatus::from(ret);

    // TODO: support column-based formatting for success/failure indicators
    if bool::from(&ret) {
        write_stderr!("[ ok ]\n");
    } else {
        let args = unescape_vec(&args[1..])?;
        if !args.is_empty() {
            let msg = args.join(" ");
            write_stderr!("{msg} ");
        }
        write_stderr!("[ !! ]\n");
    }

    Ok(ret)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "eend",
            func: run,
            help: LONG_DOC,
            usage: "eend $? [\"message\"]",
        },
        &[("0-", &[ALL])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as eend;
    use crate::macros::assert_err_re;
    use crate::pkgsh::assert_stderr;

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
