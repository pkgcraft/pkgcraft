use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkgsh::BUILD_DATA;

use super::{make_builtin, PHASE};

const LONG_DOC: &str = "\
Returns success if the USE flag argument is enabled, failure otherwise.
The return values are inverted if the flag name is prefixed with !.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let (negated, flag) = match args.len() {
        1 => match args[0].starts_with('!') {
            false => Ok((false, args[0])),
            true => Ok((true, &args[0][1..])),
        },
        n => Err(Error::Base(format!("requires 1 arg, got {n}"))),
    }?;

    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let d = d.borrow();

        if !d.iuse_effective.contains(flag) {
            return Err(Error::Base(format!("USE flag {flag:?} not in IUSE")));
        }

        let mut ret = d.use_.contains(flag);
        if negated {
            ret = !ret;
        }
        Ok(ExecStatus::from(ret))
    })
}

const USAGE: &str = "use flag";
make_builtin!("use", use_builtin, run, LONG_DOC, USAGE, &[("..", &[PHASE])]);

#[cfg(test)]
mod tests {
    use scallop::builtins::ExecStatus;

    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as use_;
    use super::*;

    builtin_scope_tests!(USAGE);

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
            assert_eq!(use_(&["use"]).unwrap(), ExecStatus::Failure(1));
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
            assert_eq!(use_(&["!use"]).unwrap(), ExecStatus::Failure(1));
        });
    }
}
