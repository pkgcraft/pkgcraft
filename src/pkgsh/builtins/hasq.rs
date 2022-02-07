use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::Result;

use super::has;

static LONG_DOC: &str = "Deprecated synonym for has.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    has::run(args)
}

pub static BUILTIN: Builtin = Builtin {
    name: "hasq",
    func: run,
    help: LONG_DOC,
    usage: "hasq needle ${haystack}",
    error_func: Some(output_error_func),
};
