use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

pub(super) fn default_phase_func(args: &[&str]) -> Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Builtin(format!("takes no args, got {}", args.len())));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let phase_func = &d.borrow().phase_func;
        match eapi.phases().get(phase_func) {
            Some(func) => func(),
            None => Err(Error::Builtin(format!("nonexistent phase function: {}", phase_func))),
        }
    })
}
