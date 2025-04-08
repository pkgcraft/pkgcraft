use scallop::ExecStatus;

use super::_new::new;
use super::doheader;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed header files into /usr/include/.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, doheader)
}

make_builtin!("newheader", newheader_builtin, true);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::io::stdin;
    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_args, cmd_scope_tests, newheader};
    use super::*;

    cmd_scope_tests!("newheader path/to/header.h new_filename");

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
            mode = 0o100644
        "#,
        );

        // re-run using data from stdin
        stdin().inject("pkgcraft").unwrap();
        newheader(&["-", "pkgcraft.h"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/include/pkgcraft.h"
            data = "pkgcraft"
            mode = 0o100644
        "#,
        );
    }
}
