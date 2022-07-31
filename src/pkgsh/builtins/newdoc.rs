use scallop::builtins::ExecStatus;
use scallop::Result;

use super::_new::new;
use super::dodoc::run as dodoc;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed documentation files.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, dodoc)
}

const USAGE: &str = "newdoc path/to/doc/file new_filename";
make_builtin!("newdoc", newdoc_builtin, run, LONG_DOC, USAGE, &[("0-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::{write_stdin, BUILD_DATA};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as newdoc;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(newdoc, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        BUILD_DATA.with(|d| d.borrow_mut().env.insert("PF".into(), "pkgcraft-0".into()));
        let file_tree = FileTree::new();

        fs::File::create("file").unwrap();
        newdoc(&["file", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/pkgcraft"
        "#,
        );

        // re-run using data from stdin
        write_stdin!("pkgcraft");
        newdoc(&["-", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/pkgcraft"
            data = "pkgcraft"
        "#,
        );
    }
}
