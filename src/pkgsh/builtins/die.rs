use std::sync::atomic::Ordering;

use nix::{sys::signal, unistd::getpid};
use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::{PkgBuiltin, ALL, NONFATAL};
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

        // TODO: send SIGTERM to background jobs (use jobs builtin)
        let pid = d.borrow().pid();
        if pid != getpid() {
            // TODO: convert error types
            signal::kill(pid, signal::Signal::SIGUSR1).expect("failed sending signal");
        }

        Ok(ExecStatus::Error)
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
        &[("0-", &[ALL])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::{run as die, BUILTIN};
    use crate::eapi::OFFICIAL_EAPIS;
    use crate::pkgsh::BUILD_DATA;

    use nix::unistd::getpid;
    use rusty_fork::rusty_fork_test;
    use scallop::{source, Shell};
    use scallop::variables::*;

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

        #[test]
        fn test_die() {
            let _sh = Shell::new("sh", Some(vec![&BUILTIN.builtin]));

            BUILD_DATA.with(|d| {
                // die in main process
                bind("VAR", "1", None, None).unwrap();
                assert_eq!(string_value("VAR").unwrap(), "1");
                source::string("die && VAR=2").unwrap();
                assert_eq!(string_value("VAR"), None);

                // die in subshell
                bind("VAR", "1", None, None).unwrap();
                assert_eq!(string_value("VAR").unwrap(), "1");
                source::string("VAR=$(die); VAR=2").unwrap();
                assert_eq!(string_value("VAR"), None);

                // verify the process hasn't changed
                assert_eq!(d.borrow().pid(), getpid());
            });
        }
    }
}
