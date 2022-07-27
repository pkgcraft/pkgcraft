use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install GNU info files into /usr/share/info/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let dest = "/usr/share/info";
        let opts = ["-m0644"];
        let install = d.borrow().install().dest(&dest)?.file_options(opts);
        install.files(args)?;
        Ok(ExecStatus::Success)
    })
}

make_builtin!("doinfo", doinfo_builtin, run, LONG_DOC, "doinfo path/to/info/file");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &["src_install"])]));

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::assert_invalid_args;
    use super::run as doinfo;
    use crate::pkgsh::test::FileTree;

    #[test]
    fn invalid_args() {
        assert_invalid_args(doinfo, &[0]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        let default_mode = 0o100644;

        fs::File::create("pkgcraft").unwrap();
        doinfo(&["pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/usr/share/info/pkgcraft"
            mode = {default_mode}
        "#
        ));
    }
}
