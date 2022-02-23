use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::_new::new;
use super::doheader::run as doheader;
use super::PkgBuiltin;

static LONG_DOC: &str = "Install renamed header files into /usr/include/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, doheader)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "newheader",
            func: run,
            help: LONG_DOC,
            usage: "newheader /path/to/header.h new_filename",
        },
        &[("5-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as newheader;

    #[test]
    fn invalid_args() {
        assert_invalid_args(newheader, &[0, 1, 3]);
    }
}
