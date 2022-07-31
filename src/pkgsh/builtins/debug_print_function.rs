use scallop::builtins::ExecStatus;
use scallop::Result;

use super::{make_builtin, ALL};

const LONG_DOC: &str = "\
Calls debug-print with $1: entering function as the first argument and the remaining arguments as
additional arguments.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

const USAGE: &str = "debug-print-function arg1 arg2";
make_builtin!(
    "debug-print-function",
    debug_print_function_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("0-", &[ALL])]
);

#[cfg(test)]
mod tests {
    use super::super::builtin_scope_tests;
    use super::*;

    builtin_scope_tests!(USAGE);

    // TODO: add usage tests
}
