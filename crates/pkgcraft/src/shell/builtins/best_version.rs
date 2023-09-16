use scallop::builtins::ExecStatus;

use super::{make_builtin, Scopes::Phases};

const LONG_DOC: &str = "Output the highest matching version of a package dependency is installed.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> scallop::Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

const USAGE: &str = "best_version cat/pkg";
make_builtin!("best_version", best_version_builtin, run, LONG_DOC, USAGE, [("..", [Phases])]);

#[cfg(test)]
mod tests {
    use super::super::builtin_scope_tests;
    use super::*;

    builtin_scope_tests!(USAGE);
}
