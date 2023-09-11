use std::sync::atomic::Ordering;

use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::eapi::Feature;
use crate::shell::{get_build_mut, write_stderr};

use super::{make_builtin, Scopes::All, NONFATAL};

const LONG_DOC: &str = "\
Displays a failure message provided in an optional argument and then aborts the build process.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let args = match args.len() {
        1 | 2 if args[0] == "-n" => {
            let eapi = get_build_mut().eapi();
            if eapi.has(Feature::NonfatalDie) {
                if NONFATAL.load(Ordering::Relaxed) {
                    if args.len() == 2 {
                        write_stderr!("{}\n", args[1])?;
                    }
                    return Ok(ExecStatus::Failure(1));
                } else {
                    return Err(Error::Base(
                        "-n option requires running under nonfatal".to_string(),
                    ));
                }
            } else {
                return Err(Error::Base(format!("-n option not supported in EAPI {eapi}")));
            }
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
    Err(Error::Base(msg.to_string()))
}

const USAGE: &str = "die \"error message\"";
make_builtin!("die", die_builtin, run, LONG_DOC, USAGE, &[("..", &[All])]);

#[cfg(test)]
mod tests {
    use scallop::source;
    use scallop::variables::{self, *};

    use crate::config::Config;
    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::pkg::BuildablePackage;
    use crate::shell::phase::PhaseKind;
    use crate::shell::{assert_stderr, BuildData, BuildState, Scope};

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

        // verify failure during build
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        for eapi in EAPIS_OFFICIAL.iter() {
            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="subshell die"
                SLOT=0
                pkg_setup() {{
                    local var=$(die subshell)
                    die main
                }}
            "#};
            let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
            BuildData::from_pkg(&pkg);
            let result = pkg.build();
            assert_err_re!(result, "die: error: subshell$");
        }
    }

    #[test]
    fn nonfatal() {
        bind("VAR", "1", None, None).unwrap();

        let build = get_build_mut();
        build.scope = Scope::Phase(PhaseKind::SrcInstall);
        build.state = BuildState::Empty(EAPIS_OFFICIAL[5]);

        // `die -n` only works in supported EAPIs
        let r = source::string("die -n");
        assert_err_re!(r, r"^die: error: -n option not supported in EAPI 5");

        build.state = BuildState::Empty(EAPIS_OFFICIAL.last().unwrap());

        // `die -n` only works as expected when run with nonfatal
        let r = source::string("die -n message");
        assert_err_re!(r, r"^die: error: -n option requires running under nonfatal");

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
