use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::dobin::install_bin;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install executables into DESTTREE/sbin.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    install_bin(args, "sbin")
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "dosbin",
            func: run,
            help: LONG_DOC,
            usage: "dosbin path/to/executable",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::fs;

    use rusty_fork::rusty_fork_test;

    use super::super::assert_invalid_args;
    use super::super::exeopts::run as exeopts;
    use super::super::into::run as into;
    use super::run as dosbin;
    use crate::macros::assert_err_re;
    use crate::pkgsh::test::FileTree;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(dosbin, &[0]);

            let _file_tree = FileTree::new();

            // nonexistent
            let r = dosbin(&["pkgcraft"]);
            assert_err_re!(r, format!("^invalid file \"pkgcraft\": .*$"));
        }

        #[test]
        fn creation() {
            let file_tree = FileTree::new();
            let default_mode = 0o100755;

            fs::File::create("pkgcraft").unwrap();
            dosbin(&["pkgcraft"]).unwrap();
            file_tree.assert(format!(r#"
                [[files]]
                path = "/usr/sbin/pkgcraft"
                mode = {default_mode}
            "#));

            // custom install dir with libopts ignored
            into(&["/"]).unwrap();
            exeopts(&["-m0777"]).unwrap();
            dosbin(&["pkgcraft"]).unwrap();
            file_tree.assert(format!(r#"
                [[files]]
                path = "/sbin/pkgcraft"
                mode = {default_mode}
            "#));
        }
    }
}
