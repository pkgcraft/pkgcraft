use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::dolib::install_lib;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install shared libraries.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    install_lib(args, Some(vec!["-m0755"]))
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "dolib.so",
            func: run,
            help: LONG_DOC,
            usage: "dolib.so path/to/lib.so",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::fs;

    use rusty_fork::rusty_fork_test;

    use super::super::assert_invalid_args;
    use super::super::libopts::run as libopts;
    use super::run as dolib_so;
    use crate::macros::assert_err_re;
    use crate::pkgsh::test::FileTree;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(dolib_so, &[0]);

            let _file_tree = FileTree::new();

            // nonexistent
            let r = dolib_so(&["pkgcraft"]);
            assert_err_re!(r, format!("^invalid file \"pkgcraft\": .*$"));
        }

        #[test]
        fn creation() {
            let file_tree = FileTree::new();
            let default_mode = 0o100755;

            fs::File::create("pkgcraft.so").unwrap();
            dolib_so(&["pkgcraft.so"]).unwrap();
            file_tree.assert(format!(r#"
                [[files]]
                path = "/usr/lib/pkgcraft.so"
                mode = {default_mode}
            "#));

            // verify libopts are ignored
            libopts(&["-m0777"]).unwrap();
            dolib_so(&["pkgcraft.so"]).unwrap();
            file_tree.assert(format!(r#"
                [[files]]
                path = "/usr/lib/pkgcraft.so"
                mode = {default_mode}
            "#));
        }
    }
}
