use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "\
Sets the options for directory creation via `dodir` and similar commands.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| {
        d.borrow_mut().diropts = args.iter().map(|s| s.to_string()).collect();
    });

    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "diropts",
            func: run,
            help: LONG_DOC,
            usage: "diropts -m0750",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use rusty_fork::rusty_fork_test;

    use super::super::assert_invalid_args;
    use super::run as diropts;
    use crate::pkgsh::BUILD_DATA;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(diropts, &[0]);
        }

        #[test]
        fn set_path() {
            diropts(&["-m0777", "-p"]).unwrap();
            BUILD_DATA.with(|d| {
                assert_eq!(d.borrow().diropts, ["-m0777", "-p"]);
            });
        }
    }
}
