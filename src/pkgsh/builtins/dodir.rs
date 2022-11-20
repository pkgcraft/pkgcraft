use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "Install directories.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let opts = &d.borrow().diropts;
        let install = d.borrow().install().dir_options(opts);
        install.dirs(args)?;
        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "dodir path/to/dir";
make_builtin!("dodir", dodir_builtin, run, LONG_DOC, USAGE, &[("0-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use crate::pkgsh::test::FileTree;

    use super::super::diropts::run as diropts;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as dodir;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dodir, &[0]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        let default_mode = 0o40755;

        for dirs in [
            vec!["dir"],
            vec!["path/to/dir"],
            vec!["/etc"],
            vec!["/usr/bin"],
            vec!["dir", "/usr/bin"],
        ] {
            dodir(&dirs).unwrap();
            let mut files = vec![];
            for dir in dirs {
                let path = dir.trim_start_matches('/');
                files.push(format!(
                    r#"
                    [[files]]
                    path = "/{path}"
                    mode = {default_mode}
                "#
                ));
            }
            file_tree.assert(files.join("\n"));
        }
    }

    #[test]
    fn custom_diropts() {
        let file_tree = FileTree::new();
        let default_mode = 0o40755;
        let custom_mode = 0o40777;

        for dir in ["dir", "/usr/bin"] {
            let path = dir.trim_start_matches('/');

            diropts(&["-m0755"]).unwrap();
            dodir(&[dir]).unwrap();
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/{path}"
                mode = {default_mode}
            "#
            ));

            // change mode and re-run dodir()
            diropts(&["-m0777"]).unwrap();
            dodir(&[dir]).unwrap();
            let path = dir.trim_start_matches('/');
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/{path}"
                mode = {custom_mode}
            "#
            ));
        }
    }
}
