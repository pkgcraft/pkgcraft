use scallop::builtins::{output_error_func, Builtin};
use scallop::Result;

use super::use_conf;

static LONG_DOC: &str = "\
Returns --with-${opt} and --without-${opt} configure flags based on a given USE flag.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<i32> {
    use_conf(args, "with", "without")
}

pub static BUILTIN: Builtin = Builtin {
    name: "use_with",
    func: run,
    help: LONG_DOC,
    usage: "use_with flag",
    error_func: Some(output_error_func),
};
