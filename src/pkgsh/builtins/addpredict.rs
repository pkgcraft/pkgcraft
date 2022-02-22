use once_cell::sync::Lazy;

use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::{PkgBuiltin, PHASE};

static LONG_DOC: &str = "Add a directory to the sandbox predict list.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "addpredict",
            func: run,
            help: LONG_DOC,
            usage: "addpredict /proc",
        },
        &[("0-", &[PHASE])],
    )
});
