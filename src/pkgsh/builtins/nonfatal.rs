use std::sync::atomic::Ordering;

use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::command::Command;
use scallop::{Error, Result};

use super::{PkgBuiltin, NONFATAL, PHASE};

const LONG_DOC: &str = "\
Takes one or more arguments and executes them as a command, preserving the exit status. If this
results in a command being called that would normally abort the build process due to a failure,
instead a non-zero exit status shall be returned.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    NONFATAL.store(true, Ordering::Relaxed);
    let orig_cmd = args.join(" ");
    let cmd = Command::new(orig_cmd, None)?;
    cmd.execute().ok();
    NONFATAL.store(false, Ordering::Relaxed);

    Ok(ExecStatus::Success)
}

make_builtin!("nonfatal", nonfatal_builtin, run, LONG_DOC, "nonfatal cmd arg1 arg2");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("4-", &[PHASE])]));

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as nonfatal;

    #[test]
    fn invalid_args() {
        assert_invalid_args(nonfatal, &[0]);
    }
}
