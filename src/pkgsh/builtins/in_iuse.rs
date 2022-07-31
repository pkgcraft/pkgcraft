use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

use super::{make_builtin, PHASE};

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

const USAGE: &str = "in_iuse flag";
make_builtin!("in_iuse", in_iuse_builtin, run, LONG_DOC, USAGE, &[("6-", &[PHASE])]);

#[cfg(test)]
mod tests {
    use scallop::builtins::ExecStatus;

    use crate::pkgsh::BUILD_DATA;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as in_iuse;
    use super::*;

    builtin_scope_tests!(USAGE);

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
        assert_eq!(in_iuse(&["use"]).unwrap(), ExecStatus::Failure(1));
    }
}
