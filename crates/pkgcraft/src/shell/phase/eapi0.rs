use std::path::Path;

use is_executable::IsExecutable;
use scallop::builtins::ExecStatus;

use crate::eapi::Feature;
use crate::shell::builtins::{econf::run as econf, emake::run as emake, unpack::run as unpack};
use crate::shell::utils::makefile_exists;
use crate::shell::BuildData;

pub(crate) fn src_unpack(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    let args: Vec<_> = build.distfiles.iter().map(|s| s.as_str()).collect();
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
    for target in ["check", "test"] {
        if emake(&[target, "-n"]).is_ok() {
            if build.eapi().has(Feature::ParallelTests) {
                emake(&[target])?;
            } else {
                emake(&["-j1", target])?;
            }
            break;
        }
    }

    Ok(ExecStatus::Success)
}
