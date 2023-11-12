use scallop::ExecStatus;

use crate::shell::write_stdout;

use super::{has, make_builtin};

const LONG_DOC: &str = "The same as has, but also prints the first argument if found.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let ret = has(args)?;
    if bool::from(&ret) {
        write_stdout!("{}", args[0])?;
    }

    Ok(ret)
}

const USAGE: &str = "hasv needle ${haystack}";
make_builtin!("hasv", hasv_builtin);

#[cfg(test)]
mod tests {
    use crate::shell::assert_stdout;

    use super::super::{assert_invalid_args, cmd_scope_tests, hasv};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(hasv, &[0]);
    }

    #[test]
    fn contains() {
        // no haystack
        assert_eq!(hasv(&["1"]).unwrap(), ExecStatus::Failure(1));
        // single element
        assert_eq!(hasv(&["1", "1"]).unwrap(), ExecStatus::Success);
        assert_stdout!("1");
        // multiple elements
        assert_eq!(hasv(&["5", "1", "2", "3", "4", "5"]).unwrap(), ExecStatus::Success);
        assert_stdout!("5");
        assert_eq!(hasv(&["6", "1", "2", "3", "4", "5"]).unwrap(), ExecStatus::Failure(1));
    }
}
