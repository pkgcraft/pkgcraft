use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
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
        let install = d.borrow().install().dest(&dest)?.file_options(opts);
        install.files(args)?;
        Ok(ExecStatus::Success)
    })
}

make_builtin!("doexe", doexe_builtin, run, LONG_DOC, "doexe path/to/executable");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &["src_install"])]));

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::assert_invalid_args;
    use super::super::exeinto::run as exeinto;
    use super::super::exeopts::run as exeopts;
    use super::run as doexe;
    use crate::pkgsh::test::FileTree;

    #[test]
    fn invalid_args() {
        assert_invalid_args(doexe, &[0]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        let default_mode = 0o100755;
        let custom_mode = 0o100777;

        fs::File::create("pkgcraft").unwrap();
        doexe(&["pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/pkgcraft"
            mode = {default_mode}
        "#
        ));

        // custom mode and install dir
        for dir in ["/opt/bin", "opt/bin"] {
            exeinto(&[dir]).unwrap();
            exeopts(&["-m0777"]).unwrap();
            doexe(&["pkgcraft"]).unwrap();
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/opt/bin/pkgcraft"
                mode = {custom_mode}
            "#
            ));
        }
    }
}
