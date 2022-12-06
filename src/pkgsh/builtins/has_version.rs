use scallop::builtins::ExecStatus;

use super::{make_builtin, PHASE};

const LONG_DOC: &str = "Determine if a package dependency is installed.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> scallop::Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

const USAGE: &str = "has_version 'cat/pkg[use]'";
make_builtin!("has_version", has_version_builtin, run, LONG_DOC, USAGE, &[("..", &[PHASE])]);

#[cfg(test)]
mod tests {
    use super::super::builtin_scope_tests;
    use super::*;

    builtin_scope_tests!(USAGE);
}
