use std::path::Path;

use is_executable::IsExecutable;
use scallop::builtins::ExecStatus;

use crate::eapi::Feature;
use crate::pkgsh::builtins::{econf::run as econf, emake::run as emake, unpack::run as unpack};
use crate::pkgsh::utils::makefile_exists;
use crate::pkgsh::BuildData;

pub(crate) fn src_unpack(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    let args: Vec<&str> = build.distfiles.iter().map(|s| s.as_str()).collect();
    if args.is_empty() {
        Ok(ExecStatus::Success)
    } else {
        unpack(&args)
    }
}

pub(crate) fn src_compile(_build: &mut BuildData) -> scallop::Result<ExecStatus> {
    if Path::new("./configure").is_executable() {
        econf(&[])?;
    }
    if makefile_exists() {
        emake(&[])?;
    }
    Ok(ExecStatus::Success)
}

pub(crate) fn src_test(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    let mut args = Vec::<&str>::new();

    if !build.eapi().has(Feature::ParallelTests) {
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
}
