use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::{PkgBuiltin, PHASE};
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "\
Returns success if the USE flag argument is enabled, failure otherwise.
The return values are inverted if the flag name is prefixed with !.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let (negated, flag) = match args.len() {
        1 => match args[0].starts_with('!') {
            false => (false, args[0]),
            true => (true, &args[0][1..]),
        },
        n => return Err(Error::Builtin(format!("requires 1 arg, got {}", n))),
    };

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let d = d.borrow();

        if !d.iuse_effective.contains(flag) {
            return Err(Error::Builtin(format!("USE flag {:?} not in IUSE", flag)));
        }

        let mut ret = d.use_.contains(flag);
        if negated {
            ret = !ret;
        }
        Ok(ExecStatus::from(ret))
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "use",
            func: run,
            help: LONG_DOC,
            usage: "use flag",
        },
        "0-",
        &[PHASE],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as use_;
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    use rusty_fork::rusty_fork_test;
    use scallop::builtins::ExecStatus;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(use_, &[0, 2]);
        }

        #[test]
        fn empty_iuse_effective() {
            assert_err_re!(use_(&["use"]), "^.* not in IUSE$");
        }

        #[test]
        fn disabled() {
            BUILD_DATA.with(|d| {
                d.borrow_mut().iuse_effective.insert("use".to_string());
                // use flag is disabled
                assert_eq!(use_(&["use"]).unwrap(), ExecStatus::Failure);
                // inverted check
                assert_eq!(use_(&["!use"]).unwrap(), ExecStatus::Success);
            });
        }

        #[test]
        fn enabled() {
            BUILD_DATA.with(|d| {
                d.borrow_mut().iuse_effective.insert("use".to_string());
                d.borrow_mut().use_.insert("use".to_string());
                // use flag is enabled
                assert_eq!(use_(&["use"]).unwrap(), ExecStatus::Success);
                // inverted check
                assert_eq!(use_(&["!use"]).unwrap(), ExecStatus::Failure);
            });
        }
    }
}
