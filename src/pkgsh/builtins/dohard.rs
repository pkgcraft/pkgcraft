use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;

static LONG_DOC: &str = "Create hard links.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let (_source, _target) = match args.len() {
        2 => (args[0], args[1]),
        n => return Err(Error::Builtin(format!("requires 2 args, got {}", n))),
    };

    // TODO: fill out this stub

    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "dohard",
            func: run,
            help: LONG_DOC,
            usage: "dohard path/to/source /path/to/target",
        },
        &[("0-3", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as dohard;

    use rusty_fork::rusty_fork_test;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(dohard, &[0, 1, 3]);
        }
    }
}
