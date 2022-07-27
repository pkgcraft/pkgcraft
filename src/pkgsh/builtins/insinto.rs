use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::variables::bind;
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::eapi::Feature;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "\
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

        if d.eapi.has(Feature::ExportInsdesttree) {
            bind("INSDESTTREE", path, None, None)?;
        }
        Ok(ExecStatus::Success)
    })
}

make_builtin!("insinto", insinto_builtin, run, LONG_DOC, "insinto /install/path");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &["src_install"])]));

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as insinto;
    use crate::pkgsh::BUILD_DATA;

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
