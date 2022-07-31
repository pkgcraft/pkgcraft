use scallop::builtins::ExecStatus;
use scallop::Result;

use super::{make_builtin, PHASE};

const LONG_DOC: &str = "Add a directory to the sandbox permitted write list.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

const USAGE: &str = "addwrite /dev";
make_builtin!("addwrite", addwrite_builtin, run, LONG_DOC, USAGE, &[("0-", &[PHASE])]);

#[cfg(test)]
mod tests {
    use super::super::builtin_scope_tests;
    use super::*;

    builtin_scope_tests!(USAGE);
}
