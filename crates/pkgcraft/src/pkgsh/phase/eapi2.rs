use is_executable::IsExecutable;
use scallop::builtins::ExecStatus;

use crate::pkgsh::builtins::{econf::run as econf, emake::run as emake};
use crate::pkgsh::utils::{configure, makefile_exists};
use crate::pkgsh::BuildData;

pub(crate) fn src_configure(_build: &mut BuildData) -> scallop::Result<ExecStatus> {
    if configure().is_executable() {
        econf(&[])
    } else {
        Ok(ExecStatus::Success)
    }
}

pub(crate) fn src_compile(_build: &mut BuildData) -> scallop::Result<ExecStatus> {
    if makefile_exists() {
        emake(&[])?;
    }
    Ok(ExecStatus::Success)
}
