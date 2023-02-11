use is_executable::IsExecutable;
use scallop::builtins::ExecStatus;

use crate::pkgsh::builtins::{econf::run as econf, emake::run as emake};
use crate::pkgsh::utils::{configure, makefile_exists};

pub(crate) fn src_configure() -> scallop::Result<ExecStatus> {
    if configure().is_executable() {
        econf(&[])
    } else {
        Ok(ExecStatus::Success)
    }
}

pub(crate) fn src_compile() -> scallop::Result<ExecStatus> {
    if makefile_exists() {
        emake(&[])?;
    }
    Ok(ExecStatus::Success)
}
