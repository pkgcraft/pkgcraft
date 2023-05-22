use std::sync::atomic::Ordering;

use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::eapi::Feature;
use crate::pkgsh::{get_build_mut, write_stderr};

use super::{make_builtin, ALL, NONFATAL};

const LONG_DOC: &str = "\
Displays a failure message provided in an optional argument and then aborts the build process.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let args = match args.len() {
        1 | 2 if get_build_mut().eapi().has(Feature::NonfatalDie) && args[0] == "-n" => {
            if NONFATAL.load(Ordering::Relaxed) {
                if args.len() == 2 {
                    write_stderr!("{}\n", args[1])?;
                }
                return Ok(ExecStatus::Failure(1));
            }
            &args[1..]
        }
        0 | 1 => args,
        n => return Err(Error::Base(format!("takes up to 1 arg, got {n}"))),
    };

    let msg = if args.is_empty() {
        "(no error message)"
    } else {
        args[0]
    };

    // TODO: add bash backtrace to output
    Err(Error::Bail(msg.to_string()))
}

const USAGE: &str = "die \"error message\"";
make_builtin!("die", die_builtin, run, LONG_DOC, USAGE, &[("..", &[ALL])]);

#[cfg(test)]
mod tests {
    use scallop::source;
    use scallop::variables::{self, *};

    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::pkgsh::phase::PhaseKind;
    use crate::pkgsh::{assert_stderr, BuildData, Scope};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as die;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(die, &[3]);

        for eapi in EAPIS_OFFICIAL
            .iter()
            .filter(|e| !e.has(Feature::NonfatalDie))
        {
            BuildData::empty(eapi);
            assert_invalid_args(die, &[2]);
        }
    }

    #[test]
    fn main() {
        bind("VAR", "1", None, None).unwrap();

        let r = source::string("die && VAR=2");
        assert_err_re!(r, r"^die: error: \(no error message\)");

        // verify bash state
        assert_eq!(variables::optional("VAR").unwrap(), "1");

        // verify message output
        let r = source::string("die \"output message\"");
        assert_err_re!(r, r"^die: error: output message");
    }

    #[test]
    #[cfg_attr(target_os = "macos", ignore)] // TODO: debug bash failures
    fn subshell() {
        bind("VAR", "1", None, None).unwrap();

        let r = source::string("FOO=$(die); VAR=2");
        assert_err_re!(r, r"^die: error: \(no error message\)");

        // verify bash state
        assert_eq!(variables::optional("VAR").unwrap(), "1");

        // verify message output
        let r = source::string("VAR=$(die \"output message\")");
        assert_err_re!(r, r"^die: error: output message");
    }

    #[test]
    fn nonfatal() {
        bind("VAR", "1", None, None).unwrap();

        let phase = PhaseKind::SrcInstall.stub();
        get_build_mut().scope = Scope::Phase(phase);

        // nonfatal requires `die -n` call
        let r = source::string("nonfatal die && VAR=2");
        assert_err_re!(r, r"^die: error: \(no error message\)");

        // nonfatal die in main process
        bind("VAR", "1", None, None).unwrap();
        source::string("nonfatal die -n message; VAR=2").unwrap();
        assert_stderr!("message\n");
        assert_eq!(variables::optional("VAR").unwrap(), "2");

        // nonfatal die in subshell without message
        bind("VAR", "1", None, None).unwrap();
        source::string("MSG=$(nonfatal die -n); VAR=2").unwrap();
        assert_eq!(variables::optional("VAR").unwrap(), "2");
        assert_eq!(variables::optional("MSG").unwrap(), "");

        // nonfatal die in subshell with message
        bind("VAR", "1", None, None).unwrap();
        source::string("MSG=$(nonfatal die -n message 2>&1); VAR=2").unwrap();
        assert_eq!(variables::optional("VAR").unwrap(), "2");
        assert_eq!(variables::optional("MSG").unwrap(), "message");
    }
}
