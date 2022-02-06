use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::Result;

static LONG_DOC: &str = "Install documentation files.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

pub static BUILTIN: Builtin = Builtin {
    name: "dodoc",
    func: run,
    help: LONG_DOC,
    usage: "dodoc [-r] doc_file",
    error_func: Some(output_error_func),
};
