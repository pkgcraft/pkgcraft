use camino::Utf8PathBuf;
use scallop::ExecStatus;

use crate::shell::environment::Variable::EXEDESTTREE;
use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(name = "doexe", long_about = "Install executables.")]
struct Command {
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let build = get_build_mut();
    let dest = build.env(EXEDESTTREE);
    let opts = &build.exeopts;
    build
        .install()
        .dest(dest)?
        .file_options(opts)
        .files(&cmd.paths)?;
    Ok(ExecStatus::Success)
}

const USAGE: &str = "doexe path/to/executable";
make_builtin!("doexe", doexe_builtin, true);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, doexe, exeinto, exeopts};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(doexe, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = doexe(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("pkgcraft").unwrap();

        doexe(&["pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/pkgcraft"
            mode = 0o100755
        "#,
        );

        // explicit root dir
        exeinto(&["/"]).unwrap();
        doexe(&["pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/pkgcraft"
            mode = 0o100755
        "#,
        );

        // custom mode and install dir
        for dir in ["/opt/bin", "opt/bin"] {
            exeinto(&[dir]).unwrap();
            exeopts(&["-m0777"]).unwrap();
            doexe(&["pkgcraft"]).unwrap();
            file_tree.assert(
                r#"
                [[files]]
                path = "/opt/bin/pkgcraft"
                mode = 0o100777
            "#,
            );
        }
    }
}
