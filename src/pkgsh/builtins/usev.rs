use std::io::Write;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::{use_::run as use_, PkgBuiltin, PHASE};
use crate::eapi::Feature;
use crate::pkgsh::{write_stdout, BUILD_DATA};

const LONG_DOC: &str = "\
The same as use, but also prints the flag name if the condition is met.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let (flag, output) = match args.len() {
            1 => {
                let output = args[0].strip_prefix('!').unwrap_or(args[0]);
                (&args[..1], output)
            }
            2 => match eapi.has(Feature::UsevTwoArgs) {
                true => (&args[..1], args[1]),
                false => return Err(Error::Builtin("requires 1 arg, got 2".into())),
            },
            n => return Err(Error::Builtin(format!("requires 1 or 2 args, got {n}"))),
        };

        let ret = use_(flag)?;
        if bool::from(&ret) {
            write_stdout!("{output}");
        }

        Ok(ret)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "usev",
            func: run,
            help: LONG_DOC,
            usage: "usev flag",
        },
        &[("0-", &[PHASE])],
    )
});

#[cfg(test)]
mod tests {
    use scallop::builtins::ExecStatus;

    use super::super::assert_invalid_args;
    use super::run as usev;
    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::pkgsh::{assert_stdout, BUILD_DATA};

    #[test]
    fn invalid_args() {
        assert_invalid_args(usev, &[0, 3]);

        BUILD_DATA.with(|d| {
            for eapi in EAPIS_OFFICIAL
                .values()
                .filter(|e| !e.has(Feature::UsevTwoArgs))
            {
                d.borrow_mut().eapi = eapi;
                assert_invalid_args(usev, &[2]);
            }
        });
    }

    #[test]
    fn empty_iuse_effective() {
        assert_err_re!(usev(&["use"]), "^.* not in IUSE$");
    }

    #[test]
    fn disabled() {
        BUILD_DATA.with(|d| {
            d.borrow_mut().iuse_effective.insert("use".to_string());

            for (args, status, expected) in
                [(&["use"], ExecStatus::Failure(1), ""), (&["!use"], ExecStatus::Success, "use")]
            {
                assert_eq!(usev(args).unwrap(), status);
                assert_stdout!(expected);
            }

            // check EAPIs that support two arg variant
            for eapi in EAPIS_OFFICIAL
                .values()
                .filter(|e| e.has(Feature::UsevTwoArgs))
            {
                d.borrow_mut().eapi = eapi;
                for (args, status, expected) in [
                    (&["use", "out"], ExecStatus::Failure(1), ""),
                    (&["!use", "out"], ExecStatus::Success, "out"),
                ] {
                    assert_eq!(usev(args).unwrap(), status);
                    assert_stdout!(expected);
                }
            }
        });
    }

    #[test]
    fn enabled() {
        BUILD_DATA.with(|d| {
            d.borrow_mut().iuse_effective.insert("use".to_string());
            d.borrow_mut().use_.insert("use".to_string());

            for (args, status, expected) in
                [(&["use"], ExecStatus::Success, "use"), (&["!use"], ExecStatus::Failure(1), "")]
            {
                assert_eq!(usev(args).unwrap(), status);
                assert_stdout!(expected);
            }

            // check EAPIs that support two arg variant
            for eapi in EAPIS_OFFICIAL
                .values()
                .filter(|e| e.has(Feature::UsevTwoArgs))
            {
                d.borrow_mut().eapi = eapi;
                for (args, status, expected) in [
                    (&["use", "out"], ExecStatus::Success, "out"),
                    (&["!use", "out"], ExecStatus::Failure(1), ""),
                ] {
                    assert_eq!(usev(args).unwrap(), status);
                    assert_stdout!(expected);
                }
            }
        });
    }
}
