use scallop::builtins::ExecStatus;
use scallop::Result;

use super::{has::run as has, make_builtin, ALL};

const LONG_DOC: &str = "Deprecated synonym for has.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    has(args)
}

const USAGE: &str = "hasq needle ${haystack}";
make_builtin!("hasq", hasq_builtin, run, LONG_DOC, USAGE, &[("0-7", &[ALL])]);

#[cfg(test)]
mod tests {
    use super::super::builtin_scope_tests;
    use super::*;

    builtin_scope_tests!(USAGE);
}
