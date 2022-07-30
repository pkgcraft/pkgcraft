use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::Result;

use super::PkgBuiltin;
use super::_default_phase_func::default_phase_func;

const LONG_DOC: &str = "\
Runs the default src_unpack implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    default_phase_func(args)
}

make_builtin!(
    "default_src_unpack",
    default_src_unpack_builtin,
    run,
    LONG_DOC,
    "default_src_unpack"
);

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("2-", &["src_unpack"])]));

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as default_src_unpack;

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_unpack, &[1]);
    }

    // TODO: add tests
}
