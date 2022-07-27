use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::Result;

use super::_new::new;
use super::doins::run as doins;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install renamed files into INSDESTREE.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, doins)
}

make_builtin!("newins", newins_builtin, run, LONG_DOC, "newins path/to/file new_filename");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &["src_install"])]));

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
