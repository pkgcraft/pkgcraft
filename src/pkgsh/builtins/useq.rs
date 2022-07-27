use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::Result;

use super::use_::run as use_;
use super::{PkgBuiltin, PHASE};

const LONG_DOC: &str = "Deprecated synonym for use.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    use_(args)
}

make_builtin!("useq", useq_builtin, run, LONG_DOC, "useq flag");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-7", &[PHASE])]));
