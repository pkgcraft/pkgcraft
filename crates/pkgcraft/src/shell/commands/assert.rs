use scallop::ExecStatus;
use scallop::array::PipeStatus;

use super::{functions::die, make_builtin};

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if PipeStatus::get().failed() {
        die(args)
    } else {
        Ok(ExecStatus::Success)
    }
}

make_builtin!("assert", assert_builtin);

#[cfg(test)]
mod tests {
    use scallop::source;
    use scallop::variables::{self, *};

    use crate::eapi::{EAPIS_OFFICIAL, Feature::NonfatalDie};
    use crate::shell::BuildData;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, functions::assert};

    cmd_scope_tests!("assert \"error message\"");

    #[test]
    fn invalid_args() {
        // make sure PIPESTATUS is set to cause failures
        source::string("true | false").ok();

        assert_invalid_cmd(assert, &[3]);

        for eapi in EAPIS_OFFICIAL.iter().filter(|e| !e.has(NonfatalDie)) {
            BuildData::empty(eapi);
            assert_invalid_cmd(assert, &[2]);
        }
    }

    #[test]
    fn success() {
        // unset PIPESTATUS
        assert!(source::string("assert").is_ok());

        // successful pipeline
        assert!(source::string("true | true; assert").is_ok());
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

    #[ignore]
    #[test]
    fn subshell() {
        bind("VAR", "1", None, None).unwrap();

        // forced subshell
        let r = source::string("(true | false; assert msg); VAR=2");
        assert_err_re!(r, "^line 1: assert: error: msg$");
        assert_eq!(variables::optional("VAR").unwrap(), "1");

        // command substitution
        let r = source::string("VAR=$(true | false; assert msg); VAR=2");
        assert_err_re!(r, "^line 1: assert: error: msg$");

        // process substitution
        let r = source::string("echo >$(true | false; assert msg); VAR=2");
        assert_err_re!(r, "^line 1: assert: error: msg$");
        assert_eq!(variables::optional("VAR").unwrap(), "1");

        // no message
        let r = source::string("VAR=$(true | false; assert)");
        assert_err_re!(r, r"^line 1: assert: error: \(no error message\)$");
    }

    #[test]
    fn nonfatal() {
        // nonfatal requires `die -n` call
        let r = source::string("true | false; nonfatal assert");
        assert_err_re!(r, r"line 1: assert: error: \(no error message\)$");

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
