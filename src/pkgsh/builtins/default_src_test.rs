use scallop::builtins::ExecStatus;
use scallop::Result;

use super::_default_phase_func::default_phase_func;
use super::make_builtin;

const LONG_DOC: &str = "\
Runs the default src_test implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    default_phase_func(args)
}

const USAGE: &str = "default_src_test";
make_builtin!(
    "default_src_test",
    default_src_test_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("2-", &["src_test"])]
);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as default_src_test;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_test, &[1]);
    }

    // TODO: add usage tests
}
