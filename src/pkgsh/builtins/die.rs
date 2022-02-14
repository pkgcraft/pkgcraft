use std::sync::atomic::Ordering;

use nix::sys::signal;
use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::shell::{error, is_subshell, kill};
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

        let msg = match args.is_empty() {
            true => "(no error message)",
            false => args[0],
        };

        // TODO: output bash backtrace
        error(format!("die called: {}", msg))?;

        // TODO: send SIGTERM to background jobs (use jobs builtin)
        match is_subshell() {
            true => {
                kill(signal::Signal::SIGUSR1)?;
                std::process::exit(2);
            }
            false => Ok(ExecStatus::Error),
        }
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
    use super::super::{assert_invalid_args, nonfatal};
    use super::{run as die, BUILTIN};
    use crate::eapi::OFFICIAL_EAPIS;
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    use nix::unistd::getpid;
    use rusty_fork::rusty_fork_test;
    use scallop::variables::*;
    use scallop::{source, Shell};

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
        fn main() {
            let sh = Shell::new("sh", Some(vec![&BUILTIN.builtin]));
            bind("VAR", "1", None, None).unwrap();

            assert_eq!(string_value("VAR").unwrap(), "1");
            let r = source::string("die && VAR=2");
            assert_err_re!(r, r"^die called: \(no error message\)");

            // verify bash state is reset
            assert_eq!(string_value("VAR"), None);

            // verify message output
            let r = source::string("die \"output message\"");
            assert_err_re!(r, r"^die called: output message");

            // verify the process hasn't changed
            assert_eq!(sh.pid(), &getpid());
        }

        #[test]
        fn subshell() {
            let sh = Shell::new("sh", Some(vec![&BUILTIN.builtin]));
            bind("VAR", "1", None, None).unwrap();

            assert_eq!(string_value("VAR").unwrap(), "1");
            let r = source::string("VAR=$(die); VAR=2");
            assert_err_re!(r, r"^die called: \(no error message\)");

            // verify bash state is reset
            assert_eq!(string_value("VAR"), None);

            // verify message output
            let r = source::string("VAR=$(die \"output message\")");
            assert_err_re!(r, r"^die called: output message");

            // verify the process hasn't changed
            assert_eq!(sh.pid(), &getpid());
        }

        #[test]
        fn nonfatal() {
            let sh = Shell::new("sh", Some(vec![&BUILTIN.builtin, &nonfatal::BUILTIN.builtin]));

            // nonfatal requires `die -n` call
            bind("VAR", "1", None, None).unwrap();
            assert_eq!(string_value("VAR").unwrap(), "1");
            let r = source::string("nonfatal die && VAR=2");
            assert_err_re!(r, r"^die called: \(no error message\)");

            // nonfatal die in main process
            bind("VAR", "1", None, None).unwrap();
            assert_eq!(string_value("VAR").unwrap(), "1");
            source::string("nonfatal die -n && VAR=2").unwrap();
            assert_eq!(string_value("VAR").unwrap(), "2");

            // nonfatal die in subshell
            bind("VAR", "1", None, None).unwrap();
            assert_eq!(string_value("VAR").unwrap(), "1");
            source::string("VAR=$(nonfatal die -n); VAR=2").unwrap();
            assert_eq!(string_value("VAR").unwrap(), "2");

            // verify the process hasn't changed
            assert_eq!(sh.pid(), &getpid());
        }
    }
}
