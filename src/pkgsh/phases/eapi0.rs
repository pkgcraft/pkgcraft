use std::path::Path;

use is_executable::IsExecutable;
use scallop::builtins::ExecStatus;
use scallop::Result;

use super::super::builtins::{econf::run as econf, emake::run as emake, unpack::run as unpack};
use crate::pkgsh::BUILD_DATA;

pub(crate) fn src_unpack() -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let distfiles = &d.borrow().distfiles;
        let args: Vec<&str> = distfiles.iter().map(|s| s.as_str()).collect();
        match !args.is_empty() {
            true => unpack(&args),
            false => Ok(ExecStatus::Success),
        }
    })
}

pub(crate) fn src_compile() -> Result<ExecStatus> {
    if Path::new("./configure").is_executable() {
        econf(&[])?;
    }

    for f in ["Makefile", "GNUmakefile", "makefile"] {
        if Path::new(f).exists() {
            emake(&[])?;
            break;
        }
    }

    Ok(ExecStatus::Success)
}
