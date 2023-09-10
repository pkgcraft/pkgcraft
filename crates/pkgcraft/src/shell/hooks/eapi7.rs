use once_cell::sync::Lazy;
use scallop::builtins::ExecStatus;

use crate::shell::phase::PhaseKind::*;
use crate::shell::BuildData;

use super::Hook;

pub(crate) static HOOKS: Lazy<Vec<Hook>> = Lazy::new(|| {
    [
        SrcInstall.pre("dostrip", dostrip_pre, 0, false),
        SrcInstall.post("dostrip", dostrip_post, 0, false),
    ]
    .into_iter()
    .collect()
});

fn dostrip_pre(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    // TODO: conditionalize based on RESTRICT value
    build.strip_include.insert("/".to_string());
    Ok(ExecStatus::Success)
}

fn dostrip_post(_build: &mut BuildData) -> scallop::Result<ExecStatus> {
    // TODO: perform dostrip operation
    Ok(ExecStatus::Success)
}
