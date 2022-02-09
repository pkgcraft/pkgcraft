use std::sync::atomic::Ordering;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::{PkgBuiltin, GLOBAL, NONFATAL};
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "\
Displays a failure message provided in an optional argument and then aborts the build process.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let args = match args.len() {
            1 | 2 if eapi.has("nonfatal_die") && args[0] == "-n" => {
                let nonfatal = NONFATAL.load(Ordering::Relaxed);
                if nonfatal {
                    if args.len() == 2 {
                        eprintln!("{}", args[1]);
                    }
                    return Ok(ExecStatus::Failure);
                }
                &args[1..]
            }
            0 | 1 => args,
            n => return Err(Error::Builtin(format!("takes up to 1 arg, got {}", n))),
        };

        if !args.is_empty() {
            eprintln!("{}", args[0]);
        }

        // TODO: output bash backtrace

        // TODO: This should send SIGTERM to the entire process group and a signal handler should be
        // used in the main process to catch that, longjmp back to the top level, and reset the library
        // instead of terminating.
        std::process::exit(1)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "die",
            func: run,
            help: LONG_DOC,
            usage: "die \"error message\"",
        },
        &[("0-", &[GLOBAL])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as die;
    use crate::eapi::OFFICIAL_EAPIS;
    use crate::pkgsh::BUILD_DATA;

    use rusty_fork::rusty_fork_test;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(die, &[3]);

            BUILD_DATA.with(|d| {
                for eapi in OFFICIAL_EAPIS.values().filter(|e| !e.has("nonfatal_die")) {
                    d.borrow_mut().eapi = eapi;
                    assert_invalid_args(die, &[2]);
                }
            });
        }
    }
}
