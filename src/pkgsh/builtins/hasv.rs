use std::io::Write;

use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::Result;

use super::{has::run as has, PkgBuiltin, ALL};
use crate::pkgsh::write_stdout;

const LONG_DOC: &str = "The same as has, but also prints the first argument if found.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let ret = has(args)?;
    if bool::from(&ret) {
        write_stdout!("{}", args[0]);
    }

    Ok(ret)
}

make_builtin!("hasv", hasv_builtin, run, LONG_DOC, "hasv needle ${haystack}");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-7", &[ALL])]));

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as hasv;

    #[test]
    fn invalid_args() {
        assert_invalid_args(hasv, &[0]);
    }
}
