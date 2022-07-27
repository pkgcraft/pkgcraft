use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::Result;

use super::PkgBuiltin;
use super::_default_phase_func::default_phase_func;

const LONG_DOC: &str = "\
Runs the default src_install implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    default_phase_func(args)
}

make_builtin!(
    "default_src_install",
    default_src_install_builtin,
    run,
    LONG_DOC,
    "default_src_install"
);

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("4-", &["src_install"])]));

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as default_src_install;

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_install, &[1]);
    }
}
