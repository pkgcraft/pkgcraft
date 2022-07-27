use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::Result;

use super::{has::run as has, PkgBuiltin, ALL};

const LONG_DOC: &str = "Deprecated synonym for has.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    has(args)
}

make_builtin!("hasq", hasq_builtin, run, LONG_DOC, "hasq needle ${haystack}");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-7", &[ALL])]));
