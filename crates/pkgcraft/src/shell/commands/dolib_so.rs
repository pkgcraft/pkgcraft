use camino::Utf8PathBuf;
use scallop::ExecStatus;

use crate::macros::build_path;
use crate::shell::environment::Variable::DESTTREE;
use crate::shell::get_build_mut;
use crate::shell::utils::get_libdir;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(name = "dolib.so", long_about = "Install shared libraries.")]
struct Command {
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;

    let build = get_build_mut();
    let libdir = get_libdir(Some("lib")).unwrap();
    let dest = build_path!(build.env(&DESTTREE), &libdir);
    build
        .install()
        .dest(dest)?
        .file_options(["-m0755"])
        .files(&cmd.paths)?;

    Ok(ExecStatus::Success)
}

make_builtin!("dolib.so", dolib_so_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use scallop::variables::{bind, unbind};

    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    use super::super::{
        assert_invalid_cmd, cmd_scope_tests,
        functions::{dolib_so, into, libopts},
    };

    cmd_scope_tests!("dolib.so path/to/lib.so");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(dolib_so, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = dolib_so(&["nonexistent"]);
        assert_err_re!(r, "^invalid file: nonexistent: No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        // force libdir default
        bind("ABI", "arch", None, None).unwrap();
        unbind("LIBDIR_arch").unwrap();

        fs::File::create("pkgcraft.so").unwrap();
        dolib_so(&["pkgcraft.so"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/lib/pkgcraft.so"
            mode = 0o100755
        "#,
        );

        // force libdir override
        bind("LIBDIR_arch", "lib64", None, None).unwrap();

        dolib_so(&["pkgcraft.so"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/lib64/pkgcraft.so"
            mode = 0o100755
        "#,
        );

        // custom install dir with libopts ignored
        into(&["/"]).unwrap();
        libopts(&["-m0777"]).unwrap();
        dolib_so(&["pkgcraft.so"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/lib64/pkgcraft.so"
            mode = 0o100755
        "#,
        );
    }
}
