use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::shell::get_build_mut;

pub(super) fn default_phase_func(args: &[&str]) -> scallop::Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Base(format!("takes no args, got {}", args.len())));
    }

    let build = get_build_mut();
    let phase = build.phase()?;
    phase.default(build)
}
