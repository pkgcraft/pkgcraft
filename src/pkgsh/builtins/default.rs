use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::{PkgBuiltin, PHASE};
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Calls the default_* function for the current phase.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Builtin(format!("takes no args, got {}", args.len())));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let phase_func = &d.borrow().phase_func;
        let builtins = d.borrow().eapi.builtins(phase_func)?;
        let default_phase_func = format!("default_{phase_func}");
        match builtins.get(default_phase_func.as_str()) {
            Some(b) => b.run(&[]),
            None => Err(Error::Builtin(format!(
                "nonexistent default phase function: {default_phase_func}",
            ))),
        }
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "default",
            func: run,
            help: LONG_DOC,
            usage: "default",
        },
        &[("2-", &[PHASE])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as default;

    #[test]
    fn invalid_args() {
        assert_invalid_args(default, &[1]);
    }
}
