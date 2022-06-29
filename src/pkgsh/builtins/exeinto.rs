use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

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

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "exeinto",
            func: run,
            help: LONG_DOC,
            usage: "exeinto /install/path",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as exeinto;
    use crate::pkgsh::BUILD_DATA;

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
