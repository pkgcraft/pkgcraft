use std::io::Write;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::{has::run as has, PkgBuiltin, ALL};
use crate::pkgsh::write_stdout;

static LONG_DOC: &str = "The same as has, but also prints the first argument if found.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let ret = has(args)?;
    if bool::from(&ret) {
        write_stdout!("{}", args[0]);
    }

    Ok(ret)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "hasv",
            func: run,
            help: LONG_DOC,
            usage: "hasv needle ${haystack}",
        },
        &[("0-7", &[ALL])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as hasv;

    #[test]
    fn invalid_args() {
        assert_invalid_args(hasv, &[0]);
    }
}
