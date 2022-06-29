use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::_new::new;
use super::doins::run as doins;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install renamed files into INSDESTREE.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, doins)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "newins",
            func: run,
            help: LONG_DOC,
            usage: "newins path/to/file new_filename",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::super::assert_invalid_args;
    use super::run as newins;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::write_stdin;

    #[test]
    fn invalid_args() {
        assert_invalid_args(newins, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("file").unwrap();
        newins(&["file", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/pkgcraft"
        "#,
        );

        // re-run using data from stdin
        write_stdin!("pkgcraft");
        newins(&["-", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/pkgcraft"
            data = "pkgcraft"
        "#,
        );
    }
}
