use scallop::builtins::ExecStatus;

use crate::pkgsh::phase::PhaseKind::SrcConfigure;

use super::_default_phase_func::default_phase_func;
use super::{make_builtin, Scopes::Phase};

const LONG_DOC: &str = "\
Runs the default src_configure implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    default_phase_func(args)
}

const USAGE: &str = "default_src_configure";
make_builtin!(
    "default_src_configure",
    default_src_configure_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("2..", &[Phase(SrcConfigure)])]
);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as default_src_configure;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_configure, &[1]);
    }

    // TODO: add usage tests
}
