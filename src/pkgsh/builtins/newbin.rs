use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::Result;

use super::_new::new;
use super::dobin::run as dobin;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install renamed executables into DESTTREE/bin.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, dobin)
}

make_builtin!("newbin", newbin_builtin, run, LONG_DOC, "newbin path/to/executable new_filename");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &["src_install"])]));

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::super::assert_invalid_args;
    use super::super::into::run as into;
    use super::run as newbin;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::write_stdin;

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
        "#,
        );
    }
}
