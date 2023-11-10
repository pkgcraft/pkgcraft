use scallop::ExecStatus;

use super::_new::new;
use super::dolib_a;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed static libraries.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, dolib_a)
}

const USAGE: &str = "newlib.a path/to/lib.a new_filename";
make_builtin!("newlib.a", newlib_a_builtin, run, LONG_DOC, USAGE, BUILTIN);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use crate::shell::test::FileTree;
    use crate::shell::write_stdin;

    use super::super::{assert_invalid_args, builtin_scope_tests, into};
    use super::BUILTIN as newlib_a;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(newlib_a, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("lib").unwrap();
        newlib_a(&["lib", "pkgcraft.a"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/lib/pkgcraft.a"
            mode = 0o100644
        "#,
        );

        // custom install dir using data from stdin
        write_stdin!("pkgcraft");
        into(&["/"]).unwrap();
        newlib_a(&["-", "pkgcraft.a"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/lib/pkgcraft.a"
            data = "pkgcraft"
            mode = 0o100644
        "#,
        );
    }
}
