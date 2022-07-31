use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "\
Sets the options for installing files via `doins` and similar commands.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| {
        d.borrow_mut().insopts = args.iter().map(|s| s.to_string()).collect();
    });

    Ok(ExecStatus::Success)
}

const USAGE: &str = "insopts -m0644";
make_builtin!("insopts", insopts_builtin, run, LONG_DOC, USAGE, &[("0-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use crate::pkgsh::BUILD_DATA;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as insopts;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(insopts, &[0]);
    }

    #[test]
    fn set_path() {
        insopts(&["-m0777", "-p"]).unwrap();
        BUILD_DATA.with(|d| {
            assert_eq!(d.borrow().insopts, ["-m0777", "-p"]);
        });
    }
}
