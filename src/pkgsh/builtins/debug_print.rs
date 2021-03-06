use scallop::builtins::ExecStatus;
use scallop::Result;

use super::{make_builtin, ALL};

const LONG_DOC: &str = "\
If in a special debug mode, the arguments should be outputted or recorded using some kind of debug
logging.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

const USAGE: &str = "debug-print msg";
make_builtin!("debug-print", debug_print_builtin, run, LONG_DOC, USAGE, &[("0-", &[ALL])]);

#[cfg(test)]
mod tests {
    use super::super::builtin_scope_tests;
    use super::*;

    builtin_scope_tests!(USAGE);

    // TODO: add usage tests
}
