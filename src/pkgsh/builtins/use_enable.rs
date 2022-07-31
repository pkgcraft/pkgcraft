use scallop::builtins::ExecStatus;
use scallop::Result;

use super::_use_conf::use_conf;
use super::{make_builtin, PHASE};

const LONG_DOC: &str = "\
Returns --enable-${opt} and --disable-${opt} configure flags based on a given USE flag.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    use_conf(args, "enable", "disable")
}

const USAGE: &str = "use_enable flag";
make_builtin!("use_enable", use_enable_builtin, run, LONG_DOC, USAGE, &[("0-", &[PHASE])]);

#[cfg(test)]
mod tests {
    use scallop::builtins::ExecStatus;

    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::pkgsh::{assert_stdout, BUILD_DATA};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as use_enable;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(use_enable, &[0, 4]);

        BUILD_DATA.with(|d| {
            for eapi in EAPIS_OFFICIAL
                .values()
                .filter(|e| !e.has(Feature::UseConfArg))
            {
                d.borrow_mut().eapi = eapi;
                assert_invalid_args(use_enable, &[3]);
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

            assert!(use_enable(&["!use"]).is_err());
            for (args, status, expected) in [
                (vec!["use"], ExecStatus::Failure(1), "--disable-use"),
                (vec!["use", "opt"], ExecStatus::Failure(1), "--disable-opt"),
                (vec!["!use", "opt"], ExecStatus::Success, "--enable-opt"),
            ] {
                assert_eq!(use_enable(&args).unwrap(), status);
                assert_stdout!(expected);
            }

            // check EAPIs that support three arg variant
            for eapi in EAPIS_OFFICIAL
                .values()
                .filter(|e| e.has(Feature::UseConfArg))
            {
                d.borrow_mut().eapi = eapi;
                for (args, status, expected) in [
                    (&["use", "opt", "val"], ExecStatus::Failure(1), "--disable-opt=val"),
                    (&["!use", "opt", "val"], ExecStatus::Success, "--enable-opt=val"),
                ] {
                    assert_eq!(use_enable(args).unwrap(), status);
                    assert_stdout!(expected);
                }
            }
        });
    }

    #[test]
    fn enabled() {
        BUILD_DATA.with(|d| {
            d.borrow_mut().iuse_effective.insert("use".to_string());
            d.borrow_mut().use_.insert("use".to_string());

            assert!(use_enable(&["!use"]).is_err());
            for (args, status, expected) in [
                (vec!["use"], ExecStatus::Success, "--enable-use"),
                (vec!["use", "opt"], ExecStatus::Success, "--enable-opt"),
                (vec!["!use", "opt"], ExecStatus::Failure(1), "--disable-opt"),
            ] {
                assert_eq!(use_enable(&args).unwrap(), status);
                assert_stdout!(expected);
            }

            // check EAPIs that support three arg variant
            for eapi in EAPIS_OFFICIAL
                .values()
                .filter(|e| e.has(Feature::UseConfArg))
            {
                d.borrow_mut().eapi = eapi;
                for (args, status, expected) in [
                    (&["use", "opt", "val"], ExecStatus::Success, "--enable-opt=val"),
                    (&["!use", "opt", "val"], ExecStatus::Failure(1), "--disable-opt=val"),
                ] {
                    assert_eq!(use_enable(args).unwrap(), status);
                    assert_stdout!(expected);
                }
            }
        });
    }
}
