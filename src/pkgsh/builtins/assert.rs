use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::variables::array_to_vec;
use scallop::Result;

use super::{die::run as die, PkgBuiltin, ALL};

const LONG_DOC: &str = "\
Calls `die` with passed arguments if any process in the most recently-executed foreground pipeline
exited with an error status.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let pipestatus = array_to_vec("PIPESTATUS").unwrap_or_default();
    match pipestatus.iter().any(|s| s != "0") {
        true => die(args),
        false => Ok(ExecStatus::Success),
    }
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "assert",
            func: run,
            help: LONG_DOC,
            usage: "assert \"error message\"",
        },
        &[("0-", &[ALL])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, nonfatal};
    use super::{run as assert, BUILTIN};
    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    use rusty_fork::rusty_fork_test;
    use scallop::variables::*;
    use scallop::{source, Shell};

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            let _sh = Shell::new("sh", None);
            // make sure PIPESTATUS is set to cause failures
            source::string("true | false").unwrap();

            assert_invalid_args(assert, &[3]);

            BUILD_DATA.with(|d| {
                for eapi in EAPIS_OFFICIAL.values().filter(|e| !e.has(Feature::NonfatalDie)) {
                    d.borrow_mut().eapi = eapi;
                    assert_invalid_args(assert, &[2]);
                }
            });
        }

        #[test]
        fn success() {
            let _sh = Shell::new("sh", None);

            // unset PIPESTATUS
            source::string("assert").unwrap();

            // successful pipeline
            source::string("true | true; assert").unwrap();
        }

        #[test]
        #[cfg_attr(target_os = "macos", ignore)] // TODO: debug shared memory failures
        fn main() {
            let _sh = Shell::new("sh", Some(vec![&BUILTIN.builtin]));
            bind("VAR", "1", None, None).unwrap();

            let r = source::string("true | false | true; assert");
            assert_err_re!(r, r"^die called: \(no error message\)");

            // verify bash state is reset
            assert_eq!(string_value("VAR"), None);

            // verify message output
            let r = source::string("true | false | true; assert \"output message\"");
            assert_err_re!(r, r"^die called: output message");
        }

        #[test]
        #[cfg_attr(target_os = "macos", ignore)] // TODO: debug shared memory failures
        fn subshell() {
            let _sh = Shell::new("sh", Some(vec![&BUILTIN.builtin]));
            bind("VAR", "1", None, None).unwrap();

            let r = source::string("VAR=$(true | false; assert); VAR=2");
            assert_err_re!(r, r"^die called: \(no error message\)");

            // verify bash state is reset
            assert_eq!(string_value("VAR"), None);

            // verify message output
            let r = source::string("VAR=$(true | false; assert \"output message\")");
            assert_err_re!(r, r"^die called: output message");
        }

        #[test]
        #[cfg_attr(target_os = "macos", ignore)] // TODO: debug shared memory failures
        fn nonfatal() {
            let _sh = Shell::new("sh", Some(vec![&BUILTIN.builtin, &nonfatal::BUILTIN.builtin]));

            // nonfatal requires `die -n` call
            let r = source::string("true | false; nonfatal assert");
            assert_err_re!(r, r"^die called: \(no error message\)");

            // nonfatal die in main process
            bind("VAR", "1", None, None).unwrap();
            source::string("true | false; nonfatal assert -n && VAR=2").unwrap();
            assert_eq!(string_value("VAR").unwrap(), "2");

            // nonfatal die in subshell
            bind("VAR", "1", None, None).unwrap();
            source::string("FOO=$(true | false; nonfatal assert -n); VAR=2").unwrap();
            assert_eq!(string_value("VAR").unwrap(), "2");
        }
    }
}
