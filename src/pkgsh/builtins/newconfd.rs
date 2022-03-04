use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::_new::new;
use super::doconfd::run as doconfd;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install renamed config files into /etc/conf.d/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, doconfd)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "newconfd",
            func: run,
            help: LONG_DOC,
            usage: "newconfd path/to/config/file new_filename",
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
    use super::run as newconfd;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::write_stdin;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(newconfd, &[0, 1, 3]);
        }

        #[test]
        fn creation() {
            let file_tree = FileTree::new();

            fs::File::create("config").unwrap();
            newconfd(&["config", "pkgcraft"]).unwrap();
            file_tree.assert(r#"
                [[files]]
                path = "/etc/conf.d/pkgcraft"
            "#);

            // re-run using data from stdin
            write_stdin!("pkgcraft");
            newconfd(&["-", "pkgcraft"]).unwrap();
            file_tree.assert(r#"
                [[files]]
                path = "/etc/conf.d/pkgcraft"
                data = "pkgcraft"
            "#);
        }
    }
}
