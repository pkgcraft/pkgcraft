use scallop::ExecStatus;

use super::make_builtin;
use super::use_;

const LONG_DOC: &str = "Deprecated synonym for use.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    use_(args)
}

const USAGE: &str = "useq flag";
make_builtin!("useq", useq_builtin);

#[cfg(test)]
mod tests {
    use super::super::builtin_scope_tests;
    use super::*;

    builtin_scope_tests!(USAGE);
}
