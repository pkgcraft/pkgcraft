use once_cell::sync::Lazy;

use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::{PkgBuiltin, PHASE};

static LONG_DOC: &str = "Add a directory to the sandbox deny list.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "adddeny",
            func: run,
            help: LONG_DOC,
            usage: "adddeny /path/to/deny",
        },
        &[("0-", &[PHASE])],
    )
});
