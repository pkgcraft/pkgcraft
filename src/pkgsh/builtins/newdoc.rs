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
    use std::fs;
    use std::io::Write;

    use rusty_fork::rusty_fork_test;

    use super::super::assert_invalid_args;
    use super::run as newdoc;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::{write_stdin, BUILD_DATA};

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(newdoc, &[0, 1, 3]);
        }

        #[test]
        fn creation() {
            BUILD_DATA.with(|d| d.borrow_mut().env.insert("PF".into(), "pkgcraft-0".into()));
            let file_tree = FileTree::new();

            fs::File::create("file").unwrap();
            newdoc(&["file", "pkgcraft"]).unwrap();
            file_tree.assert(r#"
                [[files]]
                path = "/usr/share/doc/pkgcraft-0/pkgcraft"
            "#);

            // re-run using data from stdin
            write_stdin!("pkgcraft");
            newdoc(&["-", "pkgcraft"]).unwrap();
            file_tree.assert(r#"
                [[files]]
                path = "/usr/share/doc/pkgcraft-0/pkgcraft"
                data = "pkgcraft"
            "#);
        }
    }
}
