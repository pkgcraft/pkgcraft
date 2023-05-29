use std::sync::atomic::Ordering;

use scallop::builtins::ExecStatus;
use scallop::Error;

use super::{make_builtin, Scopes::All, NONFATAL};

static LONG_DOC: &str = "\
Executed when the search for a command is unsuccessful.

This handles PATH search failures instead of using the command_not_found_handle() function method
instead.
";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let msg = format!("unknown command: {}", args[0]);
    if NONFATAL.load(Ordering::Relaxed) {
        Err(Error::Base(msg))
    } else {
        Err(Error::Bail(msg))
    }
}

make_builtin!(
    "command_not_found_handle",
    command_not_found_handle_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    &[("..", &[All])]
);
