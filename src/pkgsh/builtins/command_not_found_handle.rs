use std::sync::atomic::Ordering;

use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::{Error, Result};

use super::{PkgBuiltin, ALL, NONFATAL};

static LONG_DOC: &str = "\
Executed when the search for a command is unsuccessful.

This allows PATH search failures to be caught and handled within scallop instead of using the
command_not_found_handle() function method instead.
";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let msg = format!("unknown command: {}", args[0]);
    match NONFATAL.load(Ordering::Relaxed) {
        true => Err(Error::Base(msg)),
        false => Err(Error::Bail(msg)),
    }
}

make_builtin!(
    "command_not_found_handle",
    command_not_found_handle_builtin,
    run,
    LONG_DOC,
    "for internal use only"
);

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &[ALL])]));
