use scallop::builtins::ExecStatus;
use scallop::Result;

use super::_default_phase_func::default_phase_func;
use super::make_builtin;

const LONG_DOC: &str = "\
Runs the default src_install implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    default_phase_func(args)
}

const USAGE: &str = "default_src_install";
make_builtin!(
    "default_src_install",
    default_src_install_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("4-", &["src_install"])]
);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as default_src_install;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_install, &[1]);
    }

    // TODO: add usage tests
}
