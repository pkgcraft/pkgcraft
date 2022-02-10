use is_executable::IsExecutable;
use scallop::builtins::ExecStatus;
use scallop::Result;

use crate::pkgsh::builtins::{econf::run as econf, emake::run as emake};
use crate::pkgsh::utils::{configure, makefile_exists};

pub(crate) fn src_configure() -> Result<ExecStatus> {
    match configure().is_executable() {
        true => econf(&[]),
        false => Ok(ExecStatus::Success),
    }
}

pub(crate) fn src_compile() -> Result<ExecStatus> {
    if makefile_exists() {
        emake(&[])?;
    }
    Ok(ExecStatus::Success)
}
