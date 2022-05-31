use std::path::Path;

use is_executable::IsExecutable;
use scallop::builtins::ExecStatus;
use scallop::Result;

use crate::eapi::Feature;
use crate::pkgsh::builtins::{econf::run as econf, emake::run as emake, unpack::run as unpack};
use crate::pkgsh::utils::makefile_exists;
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
    if makefile_exists() {
        emake(&[])?;
    }
    Ok(ExecStatus::Success)
}

pub(crate) fn src_test() -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let mut args = Vec::<&str>::new();
        if !d.borrow().eapi.has(Feature::ParallelTests) {
            args.push("-j1");
        }
        for target in ["check", "test"] {
            if emake(&[target, "-n"]).is_ok() {
                args.push(target);
                emake(&args)?;
                break;
            }
        }
        Ok(ExecStatus::Success)
    })
}
