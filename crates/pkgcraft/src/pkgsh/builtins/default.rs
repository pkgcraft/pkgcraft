use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkgsh::get_build_mut;

use super::{make_builtin, Scopes::Phases};

const LONG_DOC: &str = "Calls the default_* function for the current phase.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Base(format!("takes no args, got {}", args.len())));
    }

    let build = get_build_mut();
    let builtins = build.eapi().builtins(&build.scope);
    let phase = build.phase()?;
    let default_phase = format!("default_{phase}");
    match builtins.get(default_phase.as_str()) {
        Some(b) => b.run(&[]),
        None => Err(Error::Base(format!("nonexistent default phase function: {default_phase}",))),
    }
}

const USAGE: &str = "default";
make_builtin!("default", default_builtin, run, LONG_DOC, USAGE, &[("2..", &[Phases])]);

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
