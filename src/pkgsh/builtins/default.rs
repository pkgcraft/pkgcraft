use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use super::{make_builtin, PHASE};
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Calls the default_* function for the current phase.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Builtin(format!("takes no args, got {}", args.len())));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let phase = &d.borrow().phase.unwrap();
        let builtins = d.borrow().eapi.builtins(phase);
        let default_phase = format!("default_{phase}");
        match builtins.get(default_phase.as_str()) {
            Some(b) => b.run(&[]),
            None => {
                Err(Error::Builtin(format!("nonexistent default phase function: {default_phase}",)))
            }
        }
    })
}

const USAGE: &str = "default";
make_builtin!("default", default_builtin, run, LONG_DOC, USAGE, &[("2-", &[PHASE])]);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as default;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(default, &[1]);
    }

    // TODO: add usage tests
}
