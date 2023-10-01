use scallop::ExecStatus;

use crate::shell::phase::PhaseKind::SrcInstall;

use super::_new::new;
use super::dosbin::run as dosbin;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed executables into DESTTREE/sbin.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, dosbin)
}

const USAGE: &str = "newsbin path/to/executable new_filename";
make_builtin!("newsbin", newsbin_builtin, run, LONG_DOC, USAGE, [("..", [SrcInstall])]);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use crate::shell::test::FileTree;
    use crate::shell::write_stdin;

    use super::super::into::run as into;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as newsbin;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(newsbin, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("bin").unwrap();
        newsbin(&["bin", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/sbin/pkgcraft"
            mode = 0o100755
        "#,
        );

        // custom install dir using data from stdin
        write_stdin!("pkgcraft");
        into(&["/"]).unwrap();
        newsbin(&["-", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/sbin/pkgcraft"
            data = "pkgcraft"
            mode = 0o100755
        "#,
        );
    }
}
