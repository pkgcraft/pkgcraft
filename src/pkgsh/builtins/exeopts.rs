use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "\
Sets the options for installing executables via `doexe` and similar commands.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| {
        d.borrow_mut().exeopts = args.iter().map(|s| s.to_string()).collect();
    });

    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "exeopts",
            func: run,
            help: LONG_DOC,
            usage: "exeopts -m0755",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use rusty_fork::rusty_fork_test;

    use super::super::assert_invalid_args;
    use super::run as exeopts;
    use crate::pkgsh::BUILD_DATA;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(exeopts, &[0]);
        }

        #[test]
        fn set_path() {
            exeopts(&["-m0777", "-p"]).unwrap();
            BUILD_DATA.with(|d| {
                assert_eq!(d.borrow().exeopts, ["-m0777", "-p"]);
            });
        }
    }
}
