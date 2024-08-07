use scallop::{Error, ExecStatus};

use super::dobin::install_bin;
use super::make_builtin;

const LONG_DOC: &str = "Install executables into DESTTREE/sbin.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    install_bin(args, "sbin")
}

const USAGE: &str = "dosbin /path/to/executable";
make_builtin!("dosbin", dosbin_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_args, cmd_scope_tests, dosbin, exeopts, into};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dosbin, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = dosbin(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("pkgcraft").unwrap();
        dosbin(&["pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/sbin/pkgcraft"
            mode = 0o100755
        "#,
        );

        // custom install dir with libopts ignored
        into(&["/"]).unwrap();
        exeopts(&["-m0777"]).unwrap();
        dosbin(&["pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/sbin/pkgcraft"
            mode = 0o100755
        "#,
        );
    }
}
