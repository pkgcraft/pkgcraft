use scallop::{Error, ExecStatus};

use super::debug_print;
use super::make_builtin;

const LONG_DOC: &str = "\
Calls debug-print with `now in section $*`.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let args = &[&["now in section"], args].concat();
    debug_print(args)
}

const USAGE: &str = "debug-print-section arg1 arg2";
make_builtin!("debug-print-section", debug_print_section_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests, debug_print_section};
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(debug_print_section, &[0]);
    }

    // TODO: add usage tests
}
