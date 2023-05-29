use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkgsh::get_build_mut;
use crate::pkgsh::phase::PhaseKind::SrcInstall;

use super::{make_builtin, Scopes::Phase};

const LONG_DOC: &str = "\
Sets the options for installing libraries via `dolib` and similar commands.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    get_build_mut().libopts = args.iter().map(|s| s.to_string()).collect();

    Ok(ExecStatus::Success)
}

const USAGE: &str = "libopts -m0644";
make_builtin!("libopts", libopts_builtin, run, LONG_DOC, USAGE, &[("0..7", &[Phase(SrcInstall)])]);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as libopts;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(libopts, &[0]);
    }

    #[test]
    fn set_path() {
        libopts(&["-m0777", "-p"]).unwrap();
        assert_eq!(get_build_mut().libopts, ["-m0777", "-p"]);
    }
}
