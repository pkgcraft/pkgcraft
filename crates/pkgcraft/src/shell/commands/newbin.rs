use scallop::ExecStatus;

use super::_new::new;
use super::functions::dobin;
use super::make_builtin;

// TODO: convert to clap parser
//const LONG_DOC: &str = "Install renamed executables into DESTTREE/bin.";

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, dobin)
}

make_builtin!("newbin", newbin_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::io::stdin;
    use crate::shell::test::FileTree;

    use super::super::{
        assert_invalid_args, cmd_scope_tests,
        functions::{into, newbin},
    };

    cmd_scope_tests!("newbin path/to/executable new_filename");

    #[test]
    fn invalid_args() {
        assert_invalid_args(newbin, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("bin").unwrap();
        newbin(&["bin", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/bin/pkgcraft"
            mode = 0o100755
        "#,
        );

        // custom install dir using data from stdin
        stdin().inject("pkgcraft").unwrap();
        into(&["/"]).unwrap();
        newbin(&["-", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/bin/pkgcraft"
            data = "pkgcraft"
            mode = 0o100755
        "#,
        );
    }
}
