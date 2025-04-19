use camino::Utf8PathBuf;
use scallop::ExecStatus;

use super::dobin::install_bin;
use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "dosbin",
    long_about = "Install executables into DESTTREE/sbin."
)]
struct Command {
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    install_bin(&cmd.paths, "sbin")
}

make_builtin!("dosbin", dosbin_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, dosbin, exeopts, into};

    cmd_scope_tests!("dosbin /path/to/executable");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(dosbin, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = dosbin(&["nonexistent"]);
        assert_err_re!(r, "^invalid file: nonexistent: No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("pkgcraft").unwrap();
        dosbin(&["pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/sbin/pkgcraft"
            mode = 0o100755
        "#,
        );

        // custom install dir with libopts ignored
        into(&["/"]).unwrap();
        exeopts(&["-m0777"]).unwrap();
        dosbin(&["pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/sbin/pkgcraft"
            mode = 0o100755
        "#,
        );
    }
}
