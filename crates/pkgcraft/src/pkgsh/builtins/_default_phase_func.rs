use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkgsh::BUILD_DATA;

pub(super) fn default_phase_func(args: &[&str]) -> scallop::Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Base(format!("takes no args, got {}", args.len())));
    }

    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let phase = &d.borrow().phase.expect("no running phase");
        phase.run()
    })
}
