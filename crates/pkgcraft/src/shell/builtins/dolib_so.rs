use scallop::{Error, ExecStatus};

use super::dolib::install_lib;
use super::make_builtin;

const LONG_DOC: &str = "Install shared libraries.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    install_lib(args, Some(&["-m0755"]))
}

const USAGE: &str = "dolib.so path/to/lib.so";
make_builtin!("dolib.so", dolib_so_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use scallop::variables::{bind, unbind};

    use crate::macros::assert_err_re;
    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_args, builtin_scope_tests, into, libopts};
    use super::BUILTIN as dolib_so;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dolib_so, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = dolib_so(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        // force libdir default
        bind("ABI", "arch", None, None).unwrap();
        unbind("LIBDIR_arch").unwrap();

        fs::File::create("pkgcraft.so").unwrap();
        dolib_so(&["pkgcraft.so"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/lib/pkgcraft.so"
            mode = 0o100755
        "#,
        );

        // force libdir override
        bind("LIBDIR_arch", "lib64", None, None).unwrap();

        dolib_so(&["pkgcraft.so"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/lib64/pkgcraft.so"
            mode = 0o100755
        "#,
        );

        // custom install dir with libopts ignored
        into(&["/"]).unwrap();
        libopts(&["-m0777"]).unwrap();
        dolib_so(&["pkgcraft.so"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/lib64/pkgcraft.so"
            mode = 0o100755
        "#,
        );
    }
}
