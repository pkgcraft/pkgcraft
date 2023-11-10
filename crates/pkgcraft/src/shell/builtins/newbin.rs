use scallop::ExecStatus;

use super::_new::new;
use super::dobin;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed executables into DESTTREE/bin.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, dobin)
}

const USAGE: &str = "newbin path/to/executable new_filename";
make_builtin!("newbin", newbin_builtin);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use crate::shell::test::FileTree;
    use crate::shell::write_stdin;

    use super::super::{assert_invalid_args, builtin_scope_tests, into};
    use super::BUILTIN as newbin;
    use super::*;

    builtin_scope_tests!(USAGE);

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
        write_stdin!("pkgcraft");
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
