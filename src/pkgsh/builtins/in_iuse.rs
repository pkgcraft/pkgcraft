use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::{PkgBuiltin, PHASE};
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "\
Returns success if the USE flag argument is found in IUSE_EFFECTIVE, failure otherwise.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let flag = match args.len() {
        1 => args[0],
        n => return Err(Error::Builtin(format!("requires 1 arg, got {n}"))),
    };

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let iuse_effective = &d.borrow().iuse_effective;
        Ok(ExecStatus::from(iuse_effective.contains(flag)))
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "in_iuse",
            func: run,
            help: LONG_DOC,
            usage: "in_iuse flag",
        },
        &[("6-", &[PHASE])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as in_iuse;
    use crate::pkgsh::BUILD_DATA;

    use rusty_fork::rusty_fork_test;
    use scallop::builtins::ExecStatus;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(in_iuse, &[0, 2]);
        }

        #[test]
        fn known() {
            BUILD_DATA.with(|d| {
                d.borrow_mut().iuse_effective.insert("use".to_string());
                assert_eq!(in_iuse(&["use"]).unwrap(), ExecStatus::Success);
            });
        }

        #[test]
        fn unknown() {
            assert_eq!(in_iuse(&["use"]).unwrap(), ExecStatus::Failure);
        }
    }
}
