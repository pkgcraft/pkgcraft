use scallop::builtins::ExecStatus;
use scallop::Result;

use super::_default_phase_func::default_phase_func;
use super::make_builtin;

const LONG_DOC: &str = "\
Runs the default src_prepare implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    default_phase_func(args)
}

const USAGE: &str = "default_src_prepare";
make_builtin!(
    "default_src_prepare",
    default_src_prepare_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("2-", &["src_prepare"])]
);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as default_src_prepare;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_prepare, &[1]);
    }

    // TODO: add usage tests
}
