use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

pub(super) fn default_phase_func(args: &[&str]) -> Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Base(format!("takes no args, got {}", args.len())));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let phase = &d.borrow().phase.unwrap();
        match eapi.phases().get(phase) {
            Some(phase) => phase.run(),
            None => Err(Error::Base(format!("nonexistent phase: {phase}"))),
        }
    })
}
