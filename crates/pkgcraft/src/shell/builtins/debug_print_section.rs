use scallop::{Error, ExecStatus};

use super::debug_print::run as debug_print;
use super::{make_builtin, Scopes::All};

const LONG_DOC: &str = "\
Calls debug-print with `now in section $*`.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let args = &[&["now in section"], args].concat();
    debug_print(args)
}

const USAGE: &str = "debug-print-section arg1 arg2";
make_builtin!(
    "debug-print-section",
    debug_print_section_builtin,
    run,
    LONG_DOC,
    USAGE,
    [("..", [All])]
);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as debug_print_section;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(debug_print_section, &[0]);
    }

    // TODO: add usage tests
}
