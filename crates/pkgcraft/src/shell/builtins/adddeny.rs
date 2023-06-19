use scallop::builtins::ExecStatus;

use super::{make_builtin, Scopes::Phases};

const LONG_DOC: &str = "Add a directory to the sandbox deny list.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(_args: &[&str]) -> scallop::Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

const USAGE: &str = "adddeny /path/to/deny";
make_builtin!("adddeny", adddeny_builtin, run, LONG_DOC, USAGE, &[("..", &[Phases])]);

#[cfg(test)]
mod tests {
    use super::super::builtin_scope_tests;
    use super::*;

    builtin_scope_tests!(USAGE);
}
