use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::SrcInstall;

use super::{make_builtin, Scopes::Phase};

const LONG_DOC: &str = "\
Sets the options for installing executables via `doexe` and similar commands.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    get_build_mut().exeopts = args.iter().map(|s| s.to_string()).collect();

    Ok(ExecStatus::Success)
}

const USAGE: &str = "exeopts -m0755";
make_builtin!("exeopts", exeopts_builtin, run, LONG_DOC, USAGE, &[("..", &[Phase(SrcInstall)])]);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as exeopts;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(exeopts, &[0]);
    }

    #[test]
    fn set_path() {
        exeopts(&["-m0777", "-p"]).unwrap();
        assert_eq!(get_build_mut().exeopts, ["-m0777", "-p"]);
    }
}
