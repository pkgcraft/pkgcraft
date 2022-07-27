use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Include or exclude paths for compression.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let mut d = d.borrow_mut();
        let (set, args) = match args.first() {
            Some(&"-x") => (&mut d.compress_exclude, &args[1..]),
            Some(_) => (&mut d.compress_include, args),
            None => return Err(Error::Builtin("requires 1 or more args, got 0".into())),
        };

        set.extend(args.iter().map(|s| s.to_string()));
        Ok(ExecStatus::Success)
    })
}

make_builtin!("docompress", docompress_builtin, run, LONG_DOC, "docompress [-x] /path/to/compress");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("4-", &["src_install"])]));
