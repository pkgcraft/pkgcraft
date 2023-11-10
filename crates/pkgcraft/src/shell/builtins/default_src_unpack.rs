use scallop::ExecStatus;

use super::_default_phase_func::default_phase_func;
use super::make_builtin;

const LONG_DOC: &str = "\
Runs the default src_unpack implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    default_phase_func(args)
}

const USAGE: &str = "default_src_unpack";
make_builtin!("default_src_unpack", default_src_unpack_builtin, run, LONG_DOC, USAGE, BUILTIN);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::BUILTIN as default_src_unpack;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_unpack, &[1]);
    }

    // TODO: add usage tests
}
