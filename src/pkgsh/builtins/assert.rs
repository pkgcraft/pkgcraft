use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::variables::array_to_vec;
use scallop::Result;

use super::{die::run as die, PkgBuiltin, ALL};

static LONG_DOC: &str = "\
Calls `die` with passed arguments if any process in the most recently-executed foreground pipeline
exited with an error status.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let pipestatus = array_to_vec("PIPESTATUS")?;
    let failed = pipestatus.iter().any(|s| s != "0");
    match failed {
        true => die(args),
        false => Ok(ExecStatus::Success),
    }
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "assert",
            func: run,
            help: LONG_DOC,
            usage: "assert \"error message\"",
        },
        &[("0-", &[ALL])],
    )
});
