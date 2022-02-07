use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "\
Sets the options for installing libraries via `dolib` and similar commands.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::new("requires 1 or more args, got 0"));
    }

    BUILD_DATA.with(|d| {
        d.borrow_mut().libopts = args.iter().map(|s| s.to_string()).collect();
    });

    Ok(ExecStatus::Success)
}

pub static BUILTIN: Builtin = Builtin {
    name: "libopts",
    func: run,
    help: LONG_DOC,
    usage: "libopts -m0644",
    error_func: Some(output_error_func),
};

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as libopts;

    use rusty_fork::rusty_fork_test;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(libopts, &[0]);
        }
    }
}
