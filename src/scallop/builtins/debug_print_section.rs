use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::Result;

static LONG_DOC: &str = "\
Calls debug-print with now in section $*.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    Ok(ExecStatus::Success)
}

pub static BUILTIN: Builtin = Builtin {
    name: "debug-print-section",
    func: run,
    help: LONG_DOC,
    usage: "debug-print-section arg1 arg2",
    error_func: Some(output_error_func),
};
