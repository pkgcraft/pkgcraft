use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::variables::array_to_vec;
use scallop::Result;

use super::die;

static LONG_DOC: &str = "\
Checks the value of the shell’s pipe status variable, and if any component is non-zero
(indicating failure), calls die, passing any parameters to it.";

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
