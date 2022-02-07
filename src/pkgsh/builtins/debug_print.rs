use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

static LONG_DOC: &str = "\
If in a special debug mode, the arguments should be outputted or recorded using some kind of debug
logging.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

pub static BUILTIN: Builtin = Builtin {
    name: "debug-print",
    func: run,
    help: LONG_DOC,
    usage: "debug-print msg",
};
