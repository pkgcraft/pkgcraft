use scallop::builtins::ExecStatus;
use scallop::Result;

use super::{make_builtin, ALL};

const LONG_DOC: &str = "\
Calls debug-print with now in section $*.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

const USAGE: &str = "debug-print-section arg1 arg2";
make_builtin!(
    "debug-print-section",
    debug_print_section_builtin,
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
