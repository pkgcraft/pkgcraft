use scallop::builtins::ExecStatus;
use scallop::Error;

use super::debug_print::run as debug_print;
use super::{make_builtin, ALL};

const LONG_DOC: &str = "\
Calls debug-print with `$1: entering function` as the first argument and the remaining arguments as
additional arguments.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let s = format!("{}: entering function", args[0]);
    let args = &[&[s.as_str()], &args[1..]].concat();
    debug_print(args)
}

const USAGE: &str = "debug-print-function arg1 arg2";
make_builtin!(
    "debug-print-function",
    debug_print_function_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("..", &[ALL])]
);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as debug_print_function;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(debug_print_function, &[0]);
    }

    // TODO: add usage tests
}
