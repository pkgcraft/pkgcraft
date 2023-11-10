use scallop::variables::PipeStatus;
use scallop::ExecStatus;

use super::{die, make_builtin};

const LONG_DOC: &str = "\
Calls `die` with passed arguments if any process in the most recently-executed foreground pipeline
exited with an error status.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if PipeStatus::get().failed() {
        die(args)
    } else {
        Ok(ExecStatus::Success)
    }
}

const USAGE: &str = "assert \"error message\"";
make_builtin!("assert", assert_builtin, run, LONG_DOC, USAGE, BUILTIN);

#[cfg(test)]
mod tests {
    use scallop::source;
    use scallop::variables::{self, *};

    use crate::eapi::{Feature::NonfatalDie, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::BUILTIN as assert;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        // make sure PIPESTATUS is set to cause failures
        source::string("true | false").ok();

        assert_invalid_args(assert, &[3]);

        for eapi in EAPIS_OFFICIAL.iter().filter(|e| !e.has(NonfatalDie)) {
            BuildData::empty(eapi);
            assert_invalid_args(assert, &[2]);
        }
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
        bind("VAR", "1", None, None).unwrap();

        let r = source::string("true | false | true; assert && VAR=2");
        assert_err_re!(r, r"^line 1: assert: error: \(no error message\)");

        // verify bash state
        assert_eq!(variables::optional("VAR").unwrap(), "1");

        // verify message output
        let r = source::string("true | false | true; assert \"output message\"");
        assert_err_re!(r, "^line 1: assert: error: output message");
    }

    #[test]
    #[cfg_attr(target_os = "macos", ignore)] // TODO: debug bash failures
    fn subshell() {
        bind("VAR", "1", None, None).unwrap();

        let r = source::string("FOO=$(true | false; assert); VAR=2");
        assert_err_re!(r, r"^line 1: assert: error: \(no error message\)");

        // verify bash state
        assert_eq!(variables::optional("VAR").unwrap(), "1");

        // verify message output
        let r = source::string("VAR=$(true | false; assert \"output message\")");
        assert_err_re!(r, "^line 1: assert: error: output message");
    }

    #[test]
    fn nonfatal() {
        // nonfatal requires `die -n` call
        let r = source::string("true | false; nonfatal assert");
        assert_err_re!(r, r"^line 1: assert: error: \(no error message\)");

        // nonfatal die in main process
        bind("VAR", "1", None, None).unwrap();
        source::string("true | false; nonfatal assert -n\nVAR=2").unwrap();
        assert_eq!(variables::optional("VAR").unwrap(), "2");

        // nonfatal die in subshell
        bind("VAR", "1", None, None).unwrap();
        source::string("FOO=$(true | false; nonfatal assert -n); VAR=2").unwrap();
        assert_eq!(variables::optional("VAR").unwrap(), "2");
    }
}
