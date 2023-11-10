use scallop::ExecStatus;

use super::_new::new;
use super::dolib_so;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed shared libraries.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, dolib_so)
}

const USAGE: &str = "newlib.so path/to/lib.so new_filename";
make_builtin!("newlib.so", newlib_so_builtin, run, LONG_DOC, USAGE, BUILTIN);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use crate::shell::test::FileTree;
    use crate::shell::write_stdin;

    use super::super::{assert_invalid_args, builtin_scope_tests, into};
    use super::BUILTIN as newlib_so;
    use super::*;

    builtin_scope_tests!(USAGE);

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
            mode = 0o100755
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
            mode = 0o100755
        "#,
        );
    }
}
