use scallop::builtins::ExecStatus;
use scallop::Error;

use super::{make_builtin, PHASE};

const LONG_DOC: &str = "Run sed patterns across files.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    // TODO: fill out this stub

    Ok(ExecStatus::Success)
}

const USAGE: &str = "dosed pattern file";
make_builtin!("dosed", dosed_builtin, run, LONG_DOC, USAGE, &[("0-3", &[PHASE])]);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as dosed;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dosed, &[0]);
    }
}
