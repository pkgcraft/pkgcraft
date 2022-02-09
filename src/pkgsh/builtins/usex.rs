use std::io::{stdout, Write};

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::{use_::run as use_, PkgBuiltin, PHASE};
use crate::macros::write_flush;

static LONG_DOC: &str = "\
Tests if a given USE flag is enabled and outputs a string related to its status.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let defaults = ["", "yes", "no", "", ""];
    let (flag, vals) = match args.len() {
        1 => (&args[..1], defaults),
        2..=5 => {
            // override default values with args
            let stop = args.len();
            let mut vals = defaults;
            vals[1..stop].clone_from_slice(&args[1..stop]);
            (&args[..1], vals)
        }
        n => return Err(Error::Builtin(format!("requires 1 to 5 args, got {}", n))),
    };

    match use_(flag)? {
        ExecStatus::Success => write_flush!(stdout(), "{}{}", vals[1], vals[3]),
        ExecStatus::Failure => write_flush!(stdout(), "{}{}", vals[2], vals[4]),
    }

    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "usex",
            func: run,
            help: LONG_DOC,
            usage: "usex flag",
        },
        &[("5-", &[PHASE])],
    )
});

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::super::assert_invalid_args;
    use super::run as usex;
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    use gag::BufferRedirect;
    use rusty_fork::rusty_fork_test;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(usex, &[0, 6]);
        }

        #[test]
        fn empty_iuse_effective() {
            assert_err_re!(usex(&["use"]), "^.* not in IUSE$");
        }

        #[test]
        fn disabled() {
            BUILD_DATA.with(|d| {
                d.borrow_mut().iuse_effective.insert("use".to_string());
                let mut buf = BufferRedirect::stdout().unwrap();
                for (args, expected) in [
                        (vec!["use"], "no"),
                        (vec!["use", "arg2", "arg3", "arg4", "arg5"], "arg3arg5"),
                        (vec!["!use"], "yes"),
                        (vec!["!use", "arg2", "arg3", "arg4", "arg5"], "arg2arg4"),
                        ] {
                    usex(args.as_slice()).unwrap();
                    let mut output = String::new();
                    buf.read_to_string(&mut output).unwrap();
                    assert_eq!(output, expected);
                }
            });
        }

        #[test]
        fn enabled() {
            BUILD_DATA.with(|d| {
                d.borrow_mut().iuse_effective.insert("use".to_string());
                d.borrow_mut().use_.insert("use".to_string());
                let mut buf = BufferRedirect::stdout().unwrap();
                for (args, expected) in [
                        (vec!["use"], "yes"),
                        (vec!["use", "arg2", "arg3", "arg4", "arg5"], "arg2arg4"),
                        (vec!["!use"], "no"),
                        (vec!["!use", "arg2", "arg3", "arg4", "arg5"], "arg3arg5"),
                        ] {
                    usex(args.as_slice()).unwrap();
                    let mut output = String::new();
                    buf.read_to_string(&mut output).unwrap();
                    assert_eq!(output, expected);
                }
            });
        }
    }
}
