use scallop::builtins::ExecStatus;

use crate::shell::phase::PhaseKind::SrcCompile;

use super::_default_phase_func::default_phase_func;
use super::{make_builtin, Scopes::Phase};

const LONG_DOC: &str = "\
Runs the default src_compile implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    default_phase_func(args)
}

const USAGE: &str = "default_src_compile";
make_builtin!(
    "default_src_compile",
    default_src_compile_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("2..", &[Phase(SrcCompile)])]
);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as default_src_compile;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_compile, &[1]);
    }

    // TODO: add usage tests
}
