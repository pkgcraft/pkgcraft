use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::Result;

use super::use_;

static LONG_DOC: &str = "Deprecated synonym for use.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    use_::run(args)
}

pub static BUILTIN: Builtin = Builtin {
    name: "useq",
    func: run,
    help: LONG_DOC,
    usage: "useq flag",
    error_func: Some(output_error_func),
};
