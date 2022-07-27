use std::fs::File;

use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install directories.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let install = d.borrow().install();
        // create dirs
        install.dirs(args)?;
        // create stub files
        for path in args {
            // TODO: add pkg data to file name
            let keep = install.prefix(path).join(".keep");
            File::create(&keep)
                .map_err(|e| Error::Builtin(format!("failed creating keep file: {keep:?}: {e}")))?;
        }
        Ok(ExecStatus::Success)
    })
}

make_builtin!("keepdir", keepdir_builtin, run, LONG_DOC, "keepdir path/to/kept/dir");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &["src_install"])]));

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as keepdir;
    use crate::pkgsh::test::FileTree;

    #[test]
    fn invalid_args() {
        assert_invalid_args(keepdir, &[0]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        let default_mode = 0o100644;

        for dirs in [
            vec!["dir"],
            vec!["path/to/dir"],
            vec!["/etc"],
            vec!["/usr/bin"],
            vec!["dir", "/usr/bin"],
        ] {
            keepdir(&dirs).unwrap();
            let mut files = vec![];
            for dir in dirs {
                let path = dir.trim_start_matches('/');
                files.push(format!(
                    r#"
                    [[files]]
                    path = "/{path}/.keep"
                    mode = {default_mode}
                    data = ""
                "#
                ));
            }
            file_tree.assert(files.join("\n"));
        }
    }
}
