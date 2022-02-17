use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Install documentation files.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let (_recursive, _args) = match args.first() {
            None => return Err(Error::Builtin("requires 1 or more args, got 0".into())),
            Some(&"-r") if eapi.has("dodoc_recursive") => (true, &args[1..]),
            _ => (false, args),
        };

        // TODO: fill out this stub

        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "dodoc",
            func: run,
            help: LONG_DOC,
            usage: "dodoc [-r] doc_file",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as dodoc;

    #[test]
    fn invalid_args() {
        assert_invalid_args(dodoc, &[0]);
    }
}
