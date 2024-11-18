use scallop::ExecStatus;

use super::_new::new;
use super::doins;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed files into INSDESTREE.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, doins)
}

const USAGE: &str = "newins path/to/file new_filename";
make_builtin!("newins", newins_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::io::stdin;
    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_args, cmd_scope_tests, insinto, newins};
    use super::*;

    cmd_scope_tests!(USAGE);

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
            mode = 0o100644
        "#,
        );

        // explicit root dir
        insinto(&["/"]).unwrap();
        newins(&["file", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/pkgcraft"
            mode = 0o100644
        "#,
        );

        // custom install dir
        insinto(&["/etc"]).unwrap();
        newins(&["file", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/etc/pkgcraft"
            mode = 0o100644
        "#,
        );

        // install data from stdin
        stdin().inject("pkgcraft").unwrap();
        insinto(&["/opt"]).unwrap();
        newins(&["-", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/opt/pkgcraft"
            data = "pkgcraft"
            mode = 0o100644
        "#,
        );
    }
}
