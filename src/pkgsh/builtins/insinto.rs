use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::variables::bind;
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "\
Takes exactly one argument and sets the value of INSDESTTREE.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let path = match args.len() {
        1 => match args[0] {
            "/" => "",
            s => s,
        },
        n => return Err(Error::Builtin(format!("requires 1 arg, got {n}"))),
    };

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let mut d = d.borrow_mut();
        d.insdesttree = path.to_string();

        if d.eapi.has("export_insdesttree") {
            bind("INSDESTTREE", path, None, None)?;
        }
        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "insinto",
            func: run,
            help: LONG_DOC,
            usage: "insinto /install/path",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as insinto;
    use crate::pkgsh::BUILD_DATA;

    use rusty_fork::rusty_fork_test;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(insinto, &[0]);
        }

        #[test]
        fn set_path() {
            insinto(&["/test/path"]).unwrap();
            BUILD_DATA.with(|d| {
                assert_eq!(d.borrow().insdesttree, "/test/path");
            });
        }
    }
}
