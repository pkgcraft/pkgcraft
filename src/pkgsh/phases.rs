use scallop::builtins::ExecStatus;
use scallop::Result;

pub(crate) mod eapi0;
pub(crate) mod eapi1;
pub(crate) mod eapi2;
pub(crate) mod eapi6;

pub(crate) type PhaseFn = fn() -> Result<ExecStatus>;

pub(crate) fn phase_stub() -> Result<ExecStatus> {
    Ok(ExecStatus::Success)
}
