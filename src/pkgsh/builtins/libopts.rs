use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "\
Sets the options for installing libraries via `dolib` and similar commands.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| {
        d.borrow_mut().libopts = args.iter().map(|s| s.to_string()).collect();
    });

    Ok(ExecStatus::Success)
}

make_builtin!("libopts", libopts_builtin, run, LONG_DOC, "libopts -m0644");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-6", &["src_install"])]));

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as libopts;
    use crate::pkgsh::BUILD_DATA;

    #[test]
    fn invalid_args() {
        assert_invalid_args(libopts, &[0]);
    }

    #[test]
    fn set_path() {
        libopts(&["-m0777", "-p"]).unwrap();
        BUILD_DATA.with(|d| {
            assert_eq!(d.borrow().libopts, ["-m0777", "-p"]);
        });
    }
}
