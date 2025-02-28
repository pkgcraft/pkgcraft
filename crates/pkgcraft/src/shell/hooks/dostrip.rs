use scallop::ExecStatus;

use crate::shell::BuildData;

pub(crate) fn pre(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    // TODO: conditionalize based on RESTRICT value
    build.strip_include.insert("/".into());
    Ok(ExecStatus::Success)
}

pub(crate) fn post(_build: &mut BuildData) -> scallop::Result<ExecStatus> {
    // TODO: perform dostrip operation
    Ok(ExecStatus::Success)
}
