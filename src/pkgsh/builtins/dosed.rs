use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::{PkgBuiltin, PHASE};

static LONG_DOC: &str = "Run sed patterns across files.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    // TODO: fill out this stub

    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "dosed",
            func: run,
            help: LONG_DOC,
            usage: "dosed [pattern] [file]",
        },
        &[("0-3", &[PHASE])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as dosed;

    #[test]
    fn invalid_args() {
        assert_invalid_args(dosed, &[0]);
    }
}
