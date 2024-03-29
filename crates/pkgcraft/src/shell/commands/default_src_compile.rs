use scallop::ExecStatus;

use super::_default_phase_func::default_phase_func;
use super::make_builtin;

const LONG_DOC: &str = "\
Runs the default src_compile implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    default_phase_func(args)
}

const USAGE: &str = "default_src_compile";
make_builtin!("default_src_compile", default_src_compile_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, cmd_scope_tests, default_src_compile};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_compile, &[1]);
    }

    // TODO: add usage tests
}
