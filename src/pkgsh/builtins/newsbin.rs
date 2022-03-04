use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::_new::new;
use super::dosbin::run as dosbin;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install renamed executables into DESTTREE/sbin.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, dosbin)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "newsbin",
            func: run,
            help: LONG_DOC,
            usage: "newsbin path/to/executable new_filename",
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
    use super::run as newsbin;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::write_stdin;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(newsbin, &[0, 1, 3]);
        }

        #[test]
        fn creation() {
            let file_tree = FileTree::new();

            fs::File::create("bin").unwrap();
            newsbin(&["bin", "pkgcraft"]).unwrap();
            file_tree.assert(r#"
                [[files]]
                path = "/usr/sbin/pkgcraft"
            "#);

            // custom install dir using data from stdin
            write_stdin!("pkgcraft");
            into(&["/"]).unwrap();
            newsbin(&["-", "pkgcraft"]).unwrap();
            file_tree.assert(r#"
                [[files]]
                path = "/sbin/pkgcraft"
                data = "pkgcraft"
            "#);
        }
    }
}
