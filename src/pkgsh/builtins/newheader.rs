use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::Result;

use super::_new::new;
use super::doheader::run as doheader;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install renamed header files into /usr/include/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, doheader)
}

make_builtin!(
    "newheader",
    newheader_builtin,
    run,
    LONG_DOC,
    "newheader path/to/header.h new_filename"
);

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("5-", &["src_install"])]));

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::super::assert_invalid_args;
    use super::run as newheader;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::write_stdin;

    #[test]
    fn invalid_args() {
        assert_invalid_args(newheader, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("file").unwrap();
        newheader(&["file", "pkgcraft.h"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/include/pkgcraft.h"
        "#,
        );

        // re-run using data from stdin
        write_stdin!("pkgcraft");
        newheader(&["-", "pkgcraft.h"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/include/pkgcraft.h"
            data = "pkgcraft"
        "#,
        );
    }
}
