use scallop::builtins::ExecStatus;

use super::_default_phase_func::default_phase_func;
use super::make_builtin;

const LONG_DOC: &str = "\
Runs the default pkg_nofetch implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    default_phase_func(args)
}

const USAGE: &str = "default_pkg_nofetch";
make_builtin!(
    "default_pkg_nofetch",
    default_pkg_nofetch_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("2..", &["pkg_nofetch"])]
);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as default_pkg_nofetch;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_pkg_nofetch, &[1]);
    }

    // TODO: add usage tests
}
