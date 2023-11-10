use scallop::{Error, ExecStatus};

use super::dolib::install_lib;
use super::make_builtin;

const LONG_DOC: &str = "Install static libraries.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    install_lib(args, Some(&["-m0644"]))
}

const USAGE: &str = "dolib.a path/to/lib.a";
make_builtin!("dolib.a", dolib_a_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use scallop::variables::{bind, unbind};

    use crate::macros::assert_err_re;
    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_args, builtin_scope_tests, into, libopts};
    use super::BUILTIN as dolib_a;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dolib_a, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = dolib_a(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        // force libdir default
        bind("ABI", "arch", None, None).unwrap();
        unbind("LIBDIR_arch").unwrap();

        fs::File::create("pkgcraft.a").unwrap();
        dolib_a(&["pkgcraft.a"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/lib/pkgcraft.a"
            mode = 0o100644
        "#,
        );

        // force libdir override
        bind("LIBDIR_arch", "lib64", None, None).unwrap();

        dolib_a(&["pkgcraft.a"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/lib64/pkgcraft.a"
            mode = 0o100644
        "#,
        );

        // custom install dir with libopts ignored
        into(&["/"]).unwrap();
        libopts(&["-m0755"]).unwrap();
        dolib_a(&["pkgcraft.a"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/lib64/pkgcraft.a"
            mode = 0o100644
        "#,
        );
    }
}
