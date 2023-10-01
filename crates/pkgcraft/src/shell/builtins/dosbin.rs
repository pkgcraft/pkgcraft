use scallop::{Error, ExecStatus};

use crate::shell::phase::PhaseKind::SrcInstall;

use super::dobin::install_bin;
use super::make_builtin;

const LONG_DOC: &str = "Install executables into DESTTREE/sbin.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    install_bin(args, "sbin")
}

const USAGE: &str = "dosbin /path/to/executable";
make_builtin!("dosbin", dosbin_builtin, run, LONG_DOC, USAGE, [("..", [SrcInstall])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::macros::assert_err_re;
    use crate::shell::test::FileTree;

    use super::super::exeopts::run as exeopts;
    use super::super::into::run as into;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as dosbin;
    use super::*;

    builtin_scope_tests!(USAGE);

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
        let default_mode = 0o100755;

        fs::File::create("pkgcraft").unwrap();
        dosbin(&["pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/usr/sbin/pkgcraft"
            mode = {default_mode}
        "#
        ));

        // custom install dir with libopts ignored
        into(&["/"]).unwrap();
        exeopts(&["-m0777"]).unwrap();
        dosbin(&["pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/sbin/pkgcraft"
            mode = {default_mode}
        "#
        ));
    }
}
