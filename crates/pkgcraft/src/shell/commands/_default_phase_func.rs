use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

pub(super) fn default_phase_func(args: &[&str]) -> scallop::Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Base(format!("takes no args, got {}", args.len())));
    }

    get_build_mut().phase().default()
}
