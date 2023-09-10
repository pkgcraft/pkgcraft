use once_cell::sync::Lazy;
use scallop::builtins::ExecStatus;

use crate::shell::phase::PhaseKind;
use crate::shell::BuildData;

use super::{Hook, HookKind};

pub(crate) static HOOKS: Lazy<Vec<(PhaseKind, HookKind, Vec<Hook>)>> = Lazy::new(|| {
    [
        (PhaseKind::SrcInstall, HookKind::Pre, vec![Hook::new("dostrip", dostrip_pre, 0, false)]),
        (PhaseKind::SrcInstall, HookKind::Post, vec![Hook::new("dostrip", dostrip_post, 0, false)]),
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
