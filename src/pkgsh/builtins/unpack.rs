use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::{PkgBuiltin, PHASE};

const LONG_DOC: &str = "\
Unpacks one or more source archives, in order, into the current directory.";

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
            name: "unpack",
            func: run,
            help: LONG_DOC,
            usage: "unpack file.tar.gz",
        },
        &[("0-", &[PHASE])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as unpack;

    #[test]
    fn invalid_args() {
        assert_invalid_args(unpack, &[0]);
    }
}
