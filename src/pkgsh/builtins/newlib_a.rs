use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::_new::new;
use super::dolib_a::run as dolib_a;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install renamed static libraries.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, dolib_a)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "newlib.a",
            func: run,
            help: LONG_DOC,
            usage: "newlib.a path/to/lib.a new_filename",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use rusty_fork::rusty_fork_test;

    use super::super::assert_invalid_args;
    use super::super::into::run as into;
    use super::run as newlib_a;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::write_stdin;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(newlib_a, &[0, 1, 3]);
        }

        #[test]
        fn creation() {
            let file_tree = FileTree::new();

            fs::File::create("lib").unwrap();
            newlib_a(&["lib", "pkgcraft.a"]).unwrap();
            file_tree.assert(r#"
                [[files]]
                path = "/usr/lib/pkgcraft.a"
            "#);

            // custom install dir using data from stdin
            write_stdin!("pkgcraft");
            into(&["/"]).unwrap();
            newlib_a(&["-", "pkgcraft.a"]).unwrap();
            file_tree.assert(r#"
                [[files]]
                path = "/lib/pkgcraft.a"
                data = "pkgcraft"
            "#);
        }
    }
}
