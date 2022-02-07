use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

static LONG_DOC: &str = "\
Calls debug-print with now in section $*.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

pub static BUILTIN: Builtin = Builtin {
    name: "debug-print-section",
    func: run,
    help: LONG_DOC,
    usage: "debug-print-section arg1 arg2",
};
