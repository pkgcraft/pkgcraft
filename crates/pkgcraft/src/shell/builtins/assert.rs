use scallop::builtins::ExecStatus;
use scallop::variables::PipeStatus;

use super::{die::run as die, make_builtin, Scopes::All};

const LONG_DOC: &str = "\
Calls `die` with passed arguments if any process in the most recently-executed foreground pipeline
exited with an error status.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if PipeStatus::get().failed() {
        die(args)
    } else {
        Ok(ExecStatus::Success)
    }
}

const USAGE: &str = "assert \"error message\"";
make_builtin!("assert", assert_builtin, run, LONG_DOC, USAGE, &[("..", &[All])]);

#[cfg(test)]
mod tests {
    use scallop::source;
    use scallop::variables::{self, *};

    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::shell::phase::PhaseKind;
    use crate::shell::{get_build_mut, BuildData, Scope};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as assert;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        // make sure PIPESTATUS is set to cause failures
        source::string("true | false").ok();

        assert_invalid_args(assert, &[3]);

        for eapi in EAPIS_OFFICIAL
            .iter()
            .filter(|e| !e.has(Feature::NonfatalDie))
        {
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
        assert_err_re!(r, r"^assert: error: \(no error message\)");

        // verify bash state
        assert_eq!(variables::optional("VAR").unwrap(), "1");

        // verify message output
        let r = source::string("true | false | true; assert \"output message\"");
        assert_err_re!(r, r"^assert: error: output message");
    }

    #[test]
    #[cfg_attr(target_os = "macos", ignore)] // TODO: debug bash failures
    fn subshell() {
        bind("VAR", "1", None, None).unwrap();

        let r = source::string("FOO=$(true | false; assert); VAR=2");
        assert_err_re!(r, r"^assert: error: \(no error message\)");

        // verify bash state
        assert_eq!(variables::optional("VAR").unwrap(), "1");

        // verify message output
        let r = source::string("VAR=$(true | false; assert \"output message\")");
        assert_err_re!(r, r"^assert: error: output message");
    }

    #[test]
    fn nonfatal() {
        get_build_mut().scope = Scope::Phase(PhaseKind::SrcInstall);

        // nonfatal requires `die -n` call
        let r = source::string("true | false; nonfatal assert");
        assert_err_re!(r, r"^assert: error: \(no error message\)");

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
