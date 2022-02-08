use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::{PkgBuiltin, GLOBAL};

static LONG_DOC: &str = "\
Calls debug-print with $1: entering function as the first argument and the remaining arguments as
additional arguments.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "debug-print-function",
            func: run,
            help: LONG_DOC,
            usage: "debug-print-function arg1 arg2",
        },
        "0-",
        &[GLOBAL],
    )
});
