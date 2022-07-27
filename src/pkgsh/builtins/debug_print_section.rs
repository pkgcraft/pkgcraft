use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::Result;

use super::{PkgBuiltin, ALL};

const LONG_DOC: &str = "\
Calls debug-print with now in section $*.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

make_builtin!(
    "debug-print-section",
    debug_print_section_builtin,
    run,
    LONG_DOC,
    "debug-print-section arg1 arg2"
);

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &[ALL])]));
