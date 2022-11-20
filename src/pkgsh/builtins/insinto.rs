use scallop::builtins::ExecStatus;
use scallop::variables::bind;
use scallop::Error;

use crate::eapi::Feature;
use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "\
Takes exactly one argument and sets the value of INSDESTTREE.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let path = match args.len() {
        1 => match args[0] {
            "/" => Ok(""),
            s => Ok(s),
        },
        n => Err(Error::Base(format!("requires 1 arg, got {n}"))),
    }?;

    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let mut d = d.borrow_mut();
        d.insdesttree = path.to_string();

        if d.eapi.has(Feature::ExportInsdesttree) {
            bind("INSDESTTREE", path, None, None)?;
        }
        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "insinto /install/path";
make_builtin!("insinto", insinto_builtin, run, LONG_DOC, USAGE, &[("0-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use crate::pkgsh::BUILD_DATA;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as insinto;
    use super::*;

    builtin_scope_tests!(USAGE);

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
