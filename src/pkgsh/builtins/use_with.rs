use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::_use_conf::use_conf;
use super::{PkgBuiltin, PHASE};

static LONG_DOC: &str = "\
Returns --with-${opt} and --without-${opt} configure flags based on a given USE flag.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    use_conf(args, "with", "without")
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "use_with",
            func: run,
            help: LONG_DOC,
            usage: "use_with flag",
        },
        &[("0-", &[PHASE])],
    )
});

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::super::assert_invalid_args;
    use super::run as use_with;
    use crate::eapi::OFFICIAL_EAPIS;
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    use gag::BufferRedirect;
    use rusty_fork::rusty_fork_test;
    use scallop::builtins::ExecStatus;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(use_with, &[0, 4]);

            BUILD_DATA.with(|d| {
                for eapi in OFFICIAL_EAPIS.values().filter(|e| !e.has("use_conf_arg")) {
                    d.borrow_mut().eapi = eapi;
                    assert_invalid_args(use_with, &[3]);
                }
            });
        }

        #[test]
        fn empty_iuse_effective() {
            assert_err_re!(use_with(&["use"]), "^.* not in IUSE$");
        }

        #[test]
        fn disabled() {
            BUILD_DATA.with(|d| {
                d.borrow_mut().iuse_effective.insert("use".to_string());
                let mut buf = BufferRedirect::stdout().unwrap();

                assert!(use_with(&["!use"]).is_err());
                for (args, status, expected) in [
                        (vec!["use"], ExecStatus::Failure, "--without-use"),
                        (vec!["use", "opt"], ExecStatus::Failure, "--without-opt"),
                        (vec!["!use", "opt"], ExecStatus::Success, "--with-opt"),
                        ] {
                    assert_eq!(use_with(&args).unwrap(), status);
                    let mut output = String::new();
                    buf.read_to_string(&mut output).unwrap();
                    assert_eq!(output, expected);
                }

                // check EAPIs that support three arg variant
                for eapi in OFFICIAL_EAPIS.values().filter(|e| e.has("use_conf_arg")) {
                    d.borrow_mut().eapi = eapi;
                    for (args, status, expected) in [
                            (&["use", "opt", "val"], ExecStatus::Failure, "--without-opt=val"),
                            (&["!use", "opt", "val"], ExecStatus::Success, "--with-opt=val"),
                            ] {
                        assert_eq!(use_with(args).unwrap(), status);
                        let mut output = String::new();
                        buf.read_to_string(&mut output).unwrap();
                        assert_eq!(output, expected);
                    }
                }
            });
        }

        #[test]
        fn enabled() {
            BUILD_DATA.with(|d| {
                d.borrow_mut().iuse_effective.insert("use".to_string());
                d.borrow_mut().use_.insert("use".to_string());
                let mut buf = BufferRedirect::stdout().unwrap();

                assert!(use_with(&["!use"]).is_err());
                for (args, status, expected) in [
                        (vec!["use"], ExecStatus::Success, "--with-use"),
                        (vec!["use", "opt"], ExecStatus::Success, "--with-opt"),
                        (vec!["!use", "opt"], ExecStatus::Failure, "--without-opt"),
                        ] {
                    assert_eq!(use_with(&args).unwrap(), status);
                    let mut output = String::new();
                    buf.read_to_string(&mut output).unwrap();
                    assert_eq!(output, expected);
                }

                // check EAPIs that support three arg variant
                for eapi in OFFICIAL_EAPIS.values().filter(|e| e.has("use_conf_arg")) {
                    d.borrow_mut().eapi = eapi;
                    for (args, status, expected) in [
                            (&["use", "opt", "val"], ExecStatus::Success, "--with-opt=val"),
                            (&["!use", "opt", "val"], ExecStatus::Failure, "--without-opt=val"),
                            ] {
                        assert_eq!(use_with(args).unwrap(), status);
                        let mut output = String::new();
                        buf.read_to_string(&mut output).unwrap();
                        assert_eq!(output, expected);
                    }
                }
            });
        }
    }
}
