use scallop::ExecStatus;

use super::_new::new;
use super::dodoc;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed documentation files.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, dodoc)
}

make_builtin!("newdoc", newdoc_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::io::stdin;
    use crate::shell::BuildData;
    use crate::shell::test::FileTree;
    use crate::test::test_data;

    use super::super::{assert_invalid_args, cmd_scope_tests, newdoc};

    cmd_scope_tests!("newdoc path/to/doc/file new_filename");

    #[test]
    fn invalid_args() {
        assert_invalid_args(newdoc, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        let file_tree = FileTree::new();

        fs::File::create("file").unwrap();
        newdoc(&["file", "newfile"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/newfile"
            mode = 0o100644
        "#,
        );

        // re-run using data from stdin
        stdin().inject("pkgcraft").unwrap();
        newdoc(&["-", "newfile"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/newfile"
            data = "pkgcraft"
            mode = 0o100644
        "#,
        );
    }
}
