use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::Result;

use super::{PkgBuiltin, ALL};

const LONG_DOC: &str = "\
If in a special debug mode, the arguments should be outputted or recorded using some kind of debug
logging.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

make_builtin!("debug-print", debug_print_builtin, run, LONG_DOC, "debug-print msg");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &[ALL])]));
