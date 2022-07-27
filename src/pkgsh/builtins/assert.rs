use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
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

make_builtin!("assert", assert_builtin, run, LONG_DOC, "assert \"error message\"");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &[ALL])]));

#[cfg(test)]
mod tests {
    use scallop::variables::*;
    use scallop::{builtins, source};

    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    use super::super::assert_invalid_args;
    use super::run as assert;

    #[test]
    fn invalid_args() {
        // make sure PIPESTATUS is set to cause failures
        source::string("true | false").unwrap();

        assert_invalid_args(assert, &[3]);

        BUILD_DATA.with(|d| {
            for eapi in EAPIS_OFFICIAL
                .values()
                .filter(|e| !e.has(Feature::NonfatalDie))
            {
                d.borrow_mut().eapi = eapi;
                assert_invalid_args(assert, &[2]);
            }
        });
    }

    #[test]
    fn success() {
        // unset PIPESTATUS
        source::string("assert").unwrap();

        // successful pipeline
        source::string("true | true; assert").unwrap();
    }

    #[test]
    fn main() {
        builtins::enable(&["assert"]).unwrap();
        bind("VAR", "1", None, None).unwrap();

        let r = source::string("true | false | true; assert && VAR=2");
        assert_err_re!(r, r"^assert: error: \(no error message\)");

        // verify bash state
        assert_eq!(string_value("VAR").unwrap(), "1");

        // verify message output
        let r = source::string("true | false | true; assert \"output message\"");
        assert_err_re!(r, r"^assert: error: output message");
    }

    #[test]
    #[cfg_attr(target_os = "macos", ignore)] // TODO: debug bash failures
    fn subshell() {
        builtins::enable(&["assert"]).unwrap();
        bind("VAR", "1", None, None).unwrap();

        let r = source::string("FOO=$(true | false; assert); VAR=2");
        assert_err_re!(r, r"^assert: error: \(no error message\)");

        // verify bash state
        assert_eq!(string_value("VAR").unwrap(), "1");

        // verify message output
        let r = source::string("VAR=$(true | false; assert \"output message\")");
        assert_err_re!(r, r"^assert: error: output message");
    }

    #[test]
    fn nonfatal() {
        builtins::enable(&["assert", "nonfatal"]).unwrap();

        // nonfatal requires `die -n` call
        let r = source::string("true | false; nonfatal assert");
        assert_err_re!(r, r"^assert: error: \(no error message\)");

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
