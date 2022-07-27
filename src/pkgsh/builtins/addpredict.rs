use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::Result;

use super::{PkgBuiltin, PHASE};

const LONG_DOC: &str = "Add a directory to the sandbox predict list.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

make_builtin!("addpredict", addpredict_builtin, run, LONG_DOC, "addpredict /proc");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &[PHASE])]));
