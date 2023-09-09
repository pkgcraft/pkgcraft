use scallop::builtins::ExecStatus;
use scallop::Error;

use super::{make_builtin, Scopes::All};

static LONG_DOC: &str = "\
Executed when the search for a command is unsuccessful.

This handles PATH search failures instead of using the command_not_found_handle() function method
instead.
";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    Err(Error::Base(format!("unknown command: {}", args[0])))
}

make_builtin!(
    "command_not_found_handle",
    command_not_found_handle_builtin,
    run,
    LONG_DOC,
    "for internal use only",
    &[("..", &[All])]
);
