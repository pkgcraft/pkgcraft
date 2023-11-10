use scallop::{Error, ExecStatus};

use super::make_builtin;

const LONG_DOC: &str = "\
Returns success if the first argument is found in subsequent arguments, failure otherwise.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if let [needle, haystack @ ..] = args {
        Ok(ExecStatus::from(haystack.contains(needle)))
    } else {
        Err(Error::Base("requires 1 or more args, got 0".to_string()))
    }
}

const USAGE: &str = "has needle ${haystack}";
make_builtin!("has", has_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::BUILTIN as has;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(has, &[0]);
    }

    #[test]
    fn contains() {
        // no haystack
        assert_eq!(has(&["1"]).unwrap(), ExecStatus::Failure(1));
        // single element
        assert_eq!(has(&["1", "1"]).unwrap(), ExecStatus::Success);
        // multiple elements
        assert_eq!(has(&["5", "1", "2", "3", "4", "5"]).unwrap(), ExecStatus::Success);
        assert_eq!(has(&["6", "1", "2", "3", "4", "5"]).unwrap(), ExecStatus::Failure(1));
    }
}
