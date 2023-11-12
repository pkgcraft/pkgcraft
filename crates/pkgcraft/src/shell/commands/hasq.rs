use scallop::ExecStatus;

use super::{has, make_builtin};

const LONG_DOC: &str = "Deprecated synonym for has.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    has(args)
}

const USAGE: &str = "hasq needle ${haystack}";
make_builtin!("hasq", hasq_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, cmd_scope_tests, hasq};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(hasq, &[0]);
    }

    #[test]
    fn contains() {
        // no haystack
        assert_eq!(hasq(&["1"]).unwrap(), ExecStatus::Failure(1));
        // single element
        assert_eq!(hasq(&["1", "1"]).unwrap(), ExecStatus::Success);
        // multiple elements
        assert_eq!(hasq(&["5", "1", "2", "3", "4", "5"]).unwrap(), ExecStatus::Success);
        assert_eq!(hasq(&["6", "1", "2", "3", "4", "5"]).unwrap(), ExecStatus::Failure(1));
    }
}
