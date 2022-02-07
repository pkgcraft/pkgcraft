use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::_use_conf::use_conf;

static LONG_DOC: &str = "\
Returns --enable-${opt} and --disable-${opt} configure flags based on a given USE flag.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    use_conf(args, "enable", "disable")
}

pub static BUILTIN: Builtin = Builtin {
    name: "use_enable",
    func: run,
    help: LONG_DOC,
    usage: "use_enable flag",
};

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::super::assert_invalid_args;
    use super::run as use_enable;
    use crate::eapi::OFFICIAL_EAPIS;
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    use gag::BufferRedirect;
    use rusty_fork::rusty_fork_test;
    use scallop::builtins::ExecStatus;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(use_enable, &[0, 4]);

            BUILD_DATA.with(|d| {
                for eapi in OFFICIAL_EAPIS.values() {
                    if !eapi.has("use_conf_arg") {
                        d.borrow_mut().eapi = eapi;
                        assert_invalid_args(use_enable, &[3]);
                    }
                }
            });
        }

        #[test]
        fn empty_iuse_effective() {
            assert_err_re!(use_enable(&["use"]), "^.* not in IUSE$");
        }

        #[test]
        fn disabled() {
            BUILD_DATA.with(|d| {
                d.borrow_mut().iuse_effective.insert("use".to_string());
                let mut buf = BufferRedirect::stdout().unwrap();

                assert!(use_enable(&["!use"]).is_err());
                for (args, status, expected) in [
                        (vec!["use"], ExecStatus::Failure, "--disable-use"),
                        (vec!["use", "opt"], ExecStatus::Failure, "--disable-opt"),
                        (vec!["!use", "opt"], ExecStatus::Success, "--enable-opt"),
                        ] {
                    assert_eq!(use_enable(args.as_slice()).unwrap(), status);
                    let mut output = String::new();
                    buf.read_to_string(&mut output).unwrap();
                    assert_eq!(output, expected);
                }

                // check EAPIs that support three arg variant
                for eapi in OFFICIAL_EAPIS.values() {
                    if eapi.has("use_conf_arg") {
                        d.borrow_mut().eapi = eapi;
                        for (args, status, expected) in [
                                (&["use", "opt", "val"], ExecStatus::Failure, "--disable-opt=val"),
                                (&["!use", "opt", "val"], ExecStatus::Success, "--enable-opt=val"),
                                ] {
                            assert_eq!(use_enable(args).unwrap(), status);
                            let mut output = String::new();
                            buf.read_to_string(&mut output).unwrap();
                            assert_eq!(output, expected);
                        }
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

                assert!(use_enable(&["!use"]).is_err());
                for (args, status, expected) in [
                        (vec!["use"], ExecStatus::Success, "--enable-use"),
                        (vec!["use", "opt"], ExecStatus::Success, "--enable-opt"),
                        (vec!["!use", "opt"], ExecStatus::Failure, "--disable-opt"),
                        ] {
                    assert_eq!(use_enable(args.as_slice()).unwrap(), status);
                    let mut output = String::new();
                    buf.read_to_string(&mut output).unwrap();
                    assert_eq!(output, expected);
                }

                // check EAPIs that support three arg variant
                for eapi in OFFICIAL_EAPIS.values() {
                    if eapi.has("use_conf_arg") {
                        d.borrow_mut().eapi = eapi;
                        for (args, status, expected) in [
                                (&["use", "opt", "val"], ExecStatus::Success, "--enable-opt=val"),
                                (&["!use", "opt", "val"], ExecStatus::Failure, "--disable-opt=val"),
                                ] {
                            assert_eq!(use_enable(args).unwrap(), status);
                            let mut output = String::new();
                            buf.read_to_string(&mut output).unwrap();
                            assert_eq!(output, expected);
                        }
                    }
                }
            });
        }
    }
}
