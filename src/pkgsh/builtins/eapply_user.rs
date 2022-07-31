use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

use super::{eapply::run as eapply, make_builtin};

const LONG_DOC: &str = "Apply user patches.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Builtin(format!("takes no args, got {}", args.len())));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        if !d.borrow().user_patches_applied {
            let patches = &d.borrow().user_patches;
            let args: Vec<&str> = patches.iter().map(|s| s.as_str()).collect();
            if !args.is_empty() {
                eapply(&args)?;
            }
            d.borrow_mut().user_patches_applied = true;
        }
        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "eapply_user";
make_builtin!(
    "eapply_user",
    eapply_user_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("6-", &["src_prepare"])]
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
