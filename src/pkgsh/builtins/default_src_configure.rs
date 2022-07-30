use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::Result;

use super::PkgBuiltin;
use super::_default_phase_func::default_phase_func;

const LONG_DOC: &str = "\
Runs the default src_configure implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    default_phase_func(args)
}

make_builtin!(
    "default_src_configure",
    default_src_configure_builtin,
    run,
    LONG_DOC,
    "default_src_configure"
);

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("2-", &["src_configure"])]));

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as default_src_configure;

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_configure, &[1]);
    }

    // TODO: add tests
}
