use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::{has::run as has, PkgBuiltin, GLOBAL};

static LONG_DOC: &str = "Deprecated synonym for has.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    has(args)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "hasq",
            func: run,
            help: LONG_DOC,
            usage: "hasq needle ${haystack}",
        },
        &[("0-7", &[GLOBAL])],
    )
});
