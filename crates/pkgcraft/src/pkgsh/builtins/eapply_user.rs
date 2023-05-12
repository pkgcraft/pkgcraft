use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkgsh::get_build_mut;

use super::{eapply::run as eapply, make_builtin};

const LONG_DOC: &str = "Apply user patches.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Base(format!("takes no args, got {}", args.len())));
    }

    let build = get_build_mut();

    if !build.user_patches_applied {
        let args: Vec<_> = build.user_patches.iter().map(|s| s.as_str()).collect();
        if !args.is_empty() {
            eapply(&args)?;
        }
    }

    build.user_patches_applied = true;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "eapply_user";
make_builtin!(
    "eapply_user",
    eapply_user_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("6..", &["src_prepare"])]
);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as eapply_user;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(eapply_user, &[1]);
    }

    // TODO: add tests
}
