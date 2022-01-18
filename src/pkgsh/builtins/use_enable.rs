use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::Result;

use super::_use_conf::use_conf;

static LONG_DOC: &str = "\
Returns --enable-${opt} and --disable-${opt} configure flags based on a given USE flag.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    use_conf(args, "enable", "disable")
}

pub static BUILTIN: Builtin = Builtin {
    name: "use_enable",
    func: run,
    help: LONG_DOC,
    usage: "use_enable flag",
    error_func: Some(output_error_func),
};
