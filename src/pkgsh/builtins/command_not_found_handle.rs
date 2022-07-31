use std::sync::atomic::Ordering;

use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

use super::{PkgBuiltin, ALL, ALL_BUILTINS, NONFATAL};

static LONG_DOC: &str = "\
Executed when the search for a command is unsuccessful.

This handles PATH search failures instead of using the command_not_found_handle() function method
instead.
";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let cmd = args[0];
        let scope = d.borrow().scope;
        let msg = match ALL_BUILTINS.contains_key(cmd) {
            true => format!("{scope} scope doesn't enable command: {cmd}"),
            false => format!("unknown command: {cmd}"),
        };
        match NONFATAL.load(Ordering::Relaxed) {
            true => Err(Error::Base(msg)),
            false => Err(Error::Bail(msg)),
        }
    })
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
