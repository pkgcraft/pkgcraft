use scallop::builtins::{output_error_func, Builtin};
use scallop::Result;

static LONG_DOC: &str = "\
Calls debug-print with $1: entering function as the first argument and the remaining arguments as
additional arguments.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<i32> {
    Ok(0)
}

pub static BUILTIN: Builtin = Builtin {
    name: "debug-print-function",
    func: run,
    help: LONG_DOC,
    usage: "debug-print-function arg1 arg2",
    error_func: Some(output_error_func),
};
