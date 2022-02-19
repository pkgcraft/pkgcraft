use std::process::Command;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::variables::{string_value, string_vec};
use scallop::{Error, Result};

use super::{PkgBuiltin, PHASE};
use crate::pkgsh::utils::{makefile_exists, output_command};
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Run the make command for a package.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if !makefile_exists() {
        return Err(Error::Builtin("nonexistent makefile".into()));
    }

    let make_prog = string_value("MAKE").unwrap_or_else(|| String::from("make"));
    let mut emake = Command::new(make_prog);
    if let Ok(opts) = string_vec("MAKEOPTS") {
        emake.args(&opts);
    }

    emake.args(args);
    BUILD_DATA.with(|d| output_command(&mut d.borrow_mut().stdout(), &emake));

    emake.status().map_or_else(
        |e| Err(Error::Builtin(format!("failed running: {e}"))),
        |v| Ok(ExecStatus::from(v)),
    )
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "emake",
            func: run,
            help: LONG_DOC,
            usage: "emake -C builddir",
        },
        &[("0-", &[PHASE])],
    )
});
