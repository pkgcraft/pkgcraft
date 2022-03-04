use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::_new::new;
use super::doman::run as doman;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install renamed man pages into /usr/share/man.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, doman)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "newman",
            func: run,
            help: LONG_DOC,
            usage: "newman path/to/man/page new_filename",
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
    use super::run as newman;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::write_stdin;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(newman, &[0, 1, 3]);
        }

        #[test]
        fn creation() {
            let file_tree = FileTree::new();

            fs::File::create("manpage").unwrap();
            newman(&["manpage", "pkgcraft.1"]).unwrap();
            file_tree.assert(r#"
                [[files]]
                path = "/usr/share/man/man1/pkgcraft.1"
            "#);

            // re-run using data from stdin
            write_stdin!("pkgcraft");
            newman(&["-", "pkgcraft.1"]).unwrap();
            file_tree.assert(r#"
                [[files]]
                path = "/usr/share/man/man1/pkgcraft.1"
                data = "pkgcraft"
            "#);
        }
    }
}
