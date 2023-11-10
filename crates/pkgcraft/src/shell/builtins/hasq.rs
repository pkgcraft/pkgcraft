use scallop::ExecStatus;

use super::{has, make_builtin};

const LONG_DOC: &str = "Deprecated synonym for has.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    has(args)
}

const USAGE: &str = "hasq needle ${haystack}";
make_builtin!("hasq", hasq_builtin, run, LONG_DOC, USAGE, BUILTIN);

#[cfg(test)]
mod tests {
    use super::super::builtin_scope_tests;
    use super::*;

    builtin_scope_tests!(USAGE);
}
