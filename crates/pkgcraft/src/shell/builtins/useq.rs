use scallop::builtins::ExecStatus;

use super::use_::run as use_;
use super::{make_builtin, Scopes::Phases};

const LONG_DOC: &str = "Deprecated synonym for use.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    use_(args)
}

const USAGE: &str = "useq flag";
make_builtin!("useq", useq_builtin, run, LONG_DOC, USAGE, &[("..8", &[Phases])]);

#[cfg(test)]
mod tests {
    use super::super::builtin_scope_tests;
    use super::*;

    builtin_scope_tests!(USAGE);
}
