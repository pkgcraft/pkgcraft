use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "\
Sets the options for installing executables via `doexe` and similar commands.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::new("requires 1 or more args, got 0"));
    }

    BUILD_DATA.with(|d| {
        d.borrow_mut().exeopts = args.iter().map(|s| s.to_string()).collect();
    });

    Ok(ExecStatus::Success)
}

pub static BUILTIN: Builtin = Builtin {
    name: "exeopts",
    func: run,
    help: LONG_DOC,
    usage: "exeopts -m0755",
    error_func: Some(output_error_func),
};

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as exeopts;

    use rusty_fork::rusty_fork_test;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(exeopts, &[0]);
        }
    }
}
