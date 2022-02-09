use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::PkgBuiltin;
use super::_default_phase_func::default_phase_func;

static LONG_DOC: &str = "\
Runs the default src_compile implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    default_phase_func(args)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "default_src_compile",
            func: run,
            help: LONG_DOC,
            usage: "default_src_compile",
        },
        "2-",
        &["src_compile"],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as default_src_compile;

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_compile, &[1]);
    }
}
