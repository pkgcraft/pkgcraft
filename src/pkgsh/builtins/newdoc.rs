use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::_new::new;
use super::dodoc::run as dodoc;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install renamed documentation files.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, dodoc)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "newdoc",
            func: run,
            help: LONG_DOC,
            usage: "newdoc path/to/doc/file new_filename",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as newdoc;

    #[test]
    fn invalid_args() {
        assert_invalid_args(newdoc, &[0, 1, 3]);
    }
}
