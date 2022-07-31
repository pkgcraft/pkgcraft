use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "\
Takes exactly one argument and sets the install path for doexe and newexe.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let path = match args.len() {
        1 => match args[0] {
            "/" => "",
            s => s,
        },
        n => return Err(Error::Builtin(format!("requires 1 arg, got {n}"))),
    };

    BUILD_DATA.with(|d| {
        d.borrow_mut().exedesttree = path.to_string();
    });

    Ok(ExecStatus::Success)
}

const USAGE: &str = "exeinto /install/path";
make_builtin!("exeinto", exeinto_builtin, run, LONG_DOC, USAGE, &[("0-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use crate::pkgsh::BUILD_DATA;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as exeinto;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(exeinto, &[0, 2]);
    }

    #[test]
    fn set_path() {
        exeinto(&["/test/path"]).unwrap();
        BUILD_DATA.with(|d| {
            assert_eq!(d.borrow().exedesttree, "/test/path");
        });
    }
}
