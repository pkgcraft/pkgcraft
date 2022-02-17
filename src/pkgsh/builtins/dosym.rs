use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Create symbolic links.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let (_relative, _source, _target) = match args.len() {
            3 if args[0] == "-r" && eapi.has("dosym_relative") => (true, args[1], args[2]),
            2 => (false, args[0], args[1]),
            n => return Err(Error::Builtin(format!("requires 2 args, got {}", n))),
        };

        // TODO: fill out this stub

        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "dosym",
            func: run,
            help: LONG_DOC,
            usage: "dosym path/to/source /path/to/target",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as dosym;
    use crate::eapi::OFFICIAL_EAPIS;
    use crate::pkgsh::BUILD_DATA;

    use rusty_fork::rusty_fork_test;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(dosym, &[0, 1, 4]);

            BUILD_DATA.with(|d| {
                for eapi in OFFICIAL_EAPIS.values().filter(|e| !e.has("dosym_relative")) {
                    d.borrow_mut().eapi = eapi;
                    assert_invalid_args(dosym, &[3]);
                }
            });
        }
    }
}
