use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::{PkgBuiltin, GLOBAL};

static LONG_DOC: &str = "Run the make command for a package.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "emake",
            func: run,
            help: LONG_DOC,
            usage: "emake -C builddir",
        },
        &[("0-", &[GLOBAL])],
    )
});
