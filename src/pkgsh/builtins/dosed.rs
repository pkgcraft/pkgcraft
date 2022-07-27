use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::{Error, Result};

use super::{PkgBuiltin, PHASE};

const LONG_DOC: &str = "Run sed patterns across files.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    // TODO: fill out this stub

    Ok(ExecStatus::Success)
}

make_builtin!("dosed", dosed_builtin, run, LONG_DOC, "dosed [pattern] [file]");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-3", &[PHASE])]));

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as dosed;

    #[test]
    fn invalid_args() {
        assert_invalid_args(dosed, &[0]);
    }
}
