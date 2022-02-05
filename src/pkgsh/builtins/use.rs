use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "\
Returns shell true (0) if the first argument (a USE flag name) is enabled, false otherwise. If the
flag name is prefixed with !, returns true if the flag is disabled, and false if it is enabled.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let (negated, flag) = match args.len() {
        1 => match args[0].starts_with('!') {
            false => (false, args[0]),
            true => (true, &args[0][1..]),
        },
        n => return Err(Error::new(format!("requires 1 arg, got {}", n))),
    };

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let d = d.borrow();

        if !d.iuse_effective.contains(flag) {
            return Err(Error::new(format!("USE flag {:?} not in IUSE", flag)));
        }

        let mut ret = d.r#use.contains(flag);
        if negated {
            ret = !ret;
        }
        Ok(ExecStatus::from(ret))
    })
}

pub static BUILTIN: Builtin = Builtin {
    name: "use",
    func: run,
    help: LONG_DOC,
    usage: "use flag",
    error_func: Some(output_error_func),
};

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as r#use;
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    use rusty_fork::rusty_fork_test;
    use scallop::builtins::ExecStatus;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(r#use, vec![0, 2]);
        }

        #[test]
        fn empty_iuse_effective() {
            assert_err_re!(r#use(&["use"]), "^.* not in IUSE$");
        }

        #[test]
        fn disabled() {
            BUILD_DATA.with(|d| {
                d.borrow_mut().iuse_effective.insert("use".to_string());
                // use flag is disabled
                assert_eq!(r#use(&["use"]).unwrap(), ExecStatus::Failure);
                // inverted check
                assert_eq!(r#use(&["!use"]).unwrap(), ExecStatus::Success);
            });
        }

        #[test]
        fn enabled() {
            BUILD_DATA.with(|d| {
                d.borrow_mut().iuse_effective.insert("use".to_string());
                d.borrow_mut().r#use.insert("use".to_string());
                // use flag is enabled
                assert_eq!(r#use(&["use"]).unwrap(), ExecStatus::Success);
                // inverted check
                assert_eq!(r#use(&["!use"]).unwrap(), ExecStatus::Failure);
            });
        }
    }
}
