use scallop::builtins::ExecStatus;
use scallop::Result;

use super::_new::new;
use super::doheader::run as doheader;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed header files into /usr/include/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, doheader)
}

const USAGE: &str = "newheader path/to/header.h new_filename";
make_builtin!("newheader", newheader_builtin, run, LONG_DOC, USAGE, &[("5-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::write_stdin;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as newheader;
    use super::*;

    builtin_scope_tests!(USAGE);

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
