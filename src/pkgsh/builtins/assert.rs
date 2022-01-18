use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::variables::array_to_vec;
use scallop::Result;

use super::die;

static LONG_DOC: &str = "\
Calls `die` with passed arguments if any process in the most recently-executed foreground pipeline
exited with an error status.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let pipestatus = array_to_vec("PIPESTATUS")?;
    let failed = pipestatus.iter().any(|s| s != "0");
    match failed {
        true => die::run(args),
        false => Ok(ExecStatus::Success),
    }
}

pub static BUILTIN: Builtin = Builtin {
    name: "assert",
    func: run,
    help: LONG_DOC,
    usage: "assert \"error message\"",
    error_func: Some(output_error_func),
};
