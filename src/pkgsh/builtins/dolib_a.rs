use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::dolib::install_lib;
use super::PkgBuiltin;

const LONG_DOC: &str = "Install static libraries.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    install_lib(args, Some(vec!["-m0644"]))
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "dolib.a",
            func: run,
            help: LONG_DOC,
            usage: "dolib.a path/to/lib.a",
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
    use super::run as dolib_a;
    use crate::macros::assert_err_re;
    use crate::pkgsh::test::FileTree;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(dolib_a, &[0]);

            let _file_tree = FileTree::new();

            // nonexistent
            let r = dolib_a(&["pkgcraft"]);
            assert_err_re!(r, format!("^invalid file \"pkgcraft\": .*$"));
        }

        #[test]
        fn creation() {
            let file_tree = FileTree::new();
            let default_mode = 0o100644;

            fs::File::create("pkgcraft.a").unwrap();
            dolib_a(&["pkgcraft.a"]).unwrap();
            file_tree.assert(format!(r#"
                [[files]]
                path = "/usr/lib/pkgcraft.a"
                mode = {default_mode}
            "#));

            // verify libopts are ignored
            libopts(&["-m0755"]).unwrap();
            dolib_a(&["pkgcraft.a"]).unwrap();
            file_tree.assert(format!(r#"
                [[files]]
                path = "/usr/lib/pkgcraft.a"
                mode = {default_mode}
            "#));
        }
    }
}
