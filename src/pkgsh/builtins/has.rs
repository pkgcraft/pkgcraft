use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

static LONG_DOC: &str = "\
Returns success if the first argument is found in subsequent arguments, failure otherwise.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let needle = match args.first() {
        Some(s) => s,
        None => return Err(Error::Builtin("requires 1 or more args, got 0".into())),
    };

    let haystack = &args[1..];
    Ok(ExecStatus::from(haystack.contains(needle)))
}

pub static BUILTIN: Builtin = Builtin {
    name: "has",
    func: run,
    help: LONG_DOC,
    usage: "has needle ${haystack}",
};

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as has;

    use scallop::builtins::ExecStatus;

    #[test]
    fn invalid_args() {
        assert_invalid_args(has, &[0]);
    }

    #[test]
    fn contains() {
        // no haystack
        assert_eq!(has(&["1"]).unwrap(), ExecStatus::Failure);
        // single element
        assert_eq!(has(&["1", "1"]).unwrap(), ExecStatus::Success);
        // multiple elements
        assert_eq!(
            has(&["5", "1", "2", "3", "4", "5"]).unwrap(),
            ExecStatus::Success
        );
        assert_eq!(
            has(&["6", "1", "2", "3", "4", "5"]).unwrap(),
            ExecStatus::Failure
        );
    }
}
