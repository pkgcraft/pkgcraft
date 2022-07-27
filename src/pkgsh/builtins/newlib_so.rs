use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::Result;

use super::_new::new;
use super::dolib_so::run as dolib_so;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install renamed shared libraries.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, dolib_so)
}

make_builtin!(
    "newlib.so",
    newlib_so_builtin,
    run,
    LONG_DOC,
    "newlib.so path/to/lib.so new_filename"
);

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &["src_install"])]));

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::super::assert_invalid_args;
    use super::super::into::run as into;
    use super::run as newlib_so;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::write_stdin;

    #[test]
    fn invalid_args() {
        assert_invalid_args(newlib_so, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("lib").unwrap();
        newlib_so(&["lib", "pkgcraft.so"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/lib/pkgcraft.so"
        "#,
        );

        // custom install dir using data from stdin
        write_stdin!("pkgcraft");
        into(&["/"]).unwrap();
        newlib_so(&["-", "pkgcraft.so"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/lib/pkgcraft.so"
            data = "pkgcraft"
        "#,
        );
    }
}
