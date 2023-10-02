use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::SrcInstall;

use super::make_builtin;

const LONG_DOC: &str = "Install directories.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let build = get_build_mut();
    let opts = &build.diropts;
    let install = build.install().dir_options(opts);
    install.dirs(args)?;
    Ok(ExecStatus::Success)
}

const USAGE: &str = "dodir path/to/dir";
make_builtin!("dodir", dodir_builtin, run, LONG_DOC, USAGE, [("..", [SrcInstall])]);

#[cfg(test)]
mod tests {
    use crate::shell::test::FileTree;

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
                    mode = 0o40755
                "#
                ));
            }
            file_tree.assert(files.join("\n"));
        }
    }

    #[test]
    fn custom_diropts() {
        let file_tree = FileTree::new();

        for dir in ["dir", "/usr/bin"] {
            let path = dir.trim_start_matches('/');

            diropts(&["-m0755"]).unwrap();
            dodir(&[dir]).unwrap();
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/{path}"
                mode = 0o40755
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
                mode = 0o40777
            "#
            ));
        }
    }
}
