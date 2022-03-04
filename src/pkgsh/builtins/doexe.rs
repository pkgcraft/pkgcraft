use std::path::Path;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install executables.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let dest = &d.borrow().exedesttree;
        let opts = &d.borrow().exeopts;
        let install = d.borrow().install().dest(&dest)?.ins_options(opts);

        let files = args
            .iter()
            .map(Path::new)
            .filter_map(|f| f.file_name().map(|name| (f, name)));
        install.files(files)?;

        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "doexe",
            func: run,
            help: LONG_DOC,
            usage: "doexe path/to/executable",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::fs;

    use rusty_fork::rusty_fork_test;

    use super::super::assert_invalid_args;
    use super::super::exeinto::run as exeinto;
    use super::super::exeopts::run as exeopts;
    use super::run as doexe;
    use crate::macros::assert_err_re;
    use crate::pkgsh::test::FileTree;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(doexe, &[0]);

            let _file_tree = FileTree::new();

            // nonexistent
            let r = doexe(&["pkgcraft"]);
            assert_err_re!(r, format!("^invalid file \"pkgcraft\": .*$"));
        }

        #[test]
        fn creation() {
            let file_tree = FileTree::new();
            let default_mode = 0o100755;
            let custom_mode = 0o100777;

            fs::File::create("pkgcraft").unwrap();
            doexe(&["pkgcraft"]).unwrap();
            file_tree.assert(format!(r#"
                [[files]]
                path = "/pkgcraft"
                mode = {default_mode}
            "#));

            // custom mode and install dir
            exeinto(&["/opt/bin"]).unwrap();
            exeopts(&["-m0777"]).unwrap();
            doexe(&["pkgcraft"]).unwrap();
            file_tree.assert(format!(r#"
                [[files]]
                path = "/opt/bin/pkgcraft"
                mode = {custom_mode}
            "#));
        }
    }
}
