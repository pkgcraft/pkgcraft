use camino::Utf8PathBuf;
use nix::unistd::geteuid;
use scallop::ExecStatus;

use crate::macros::build_path;
use crate::shell::environment::Variable::DESTTREE;
use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "dobin",
    disable_help_flag = true,
    long_about = "Install executables into DESTTREE/bin."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(required = true, allow_hyphen_values = true, value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,
}

pub(super) fn install_bin(paths: &[Utf8PathBuf], dest: &str) -> scallop::Result<ExecStatus> {
    let build = get_build_mut();
    let dest = build_path!(build.env(DESTTREE), dest);
    let opts: &[&str] = if geteuid().is_root() {
        &["-m0755", "-o", "root", "-g", "root"]
    } else {
        &["-m0755"]
    };
    build
        .install()
        .dest(dest)?
        .file_options(opts)
        .files(paths)?;
    Ok(ExecStatus::Success)
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    install_bin(&cmd.paths, "bin")
}

make_builtin!("dobin", dobin_builtin, true);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, dobin, exeopts, into};

    cmd_scope_tests!("dobin path/to/executable");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(dobin, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = dobin(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        fs::File::create("file").unwrap();

        dobin(&["file"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/bin/file"
            mode = 0o100755
        "#,
        );

        into(&["-"]).unwrap();
        dobin(&["file"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/-/bin/file"
            mode = 0o100755
        "#,
        );

        // custom install dir with libopts ignored
        into(&["/"]).unwrap();
        exeopts(&["-m0777"]).unwrap();
        dobin(&["file"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/bin/file"
            mode = 0o100755
        "#,
        );
    }
}
