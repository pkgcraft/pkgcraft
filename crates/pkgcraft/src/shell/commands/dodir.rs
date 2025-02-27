use camino::Utf8PathBuf;
use scallop::ExecStatus;

use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(name = "dodir", long_about = "Install directories.")]
struct Command {
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let build = get_build_mut();
    let opts = &build.diropts;
    build.install().dir_options(opts).dirs(cmd.paths)?;
    Ok(ExecStatus::Success)
}

const USAGE: &str = "dodir path/to/dir";
make_builtin!("dodir", dodir_builtin);

#[cfg(test)]
mod tests {
    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, diropts, dodir};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(dodir, &[0]);
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
