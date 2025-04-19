use camino::Utf8PathBuf;
use scallop::{Error, ExecStatus};

use crate::files::NO_WALKDIR_FILTER;
use crate::shell::environment::Variable::INSDESTTREE;
use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "doins",
    disable_help_flag = true,
    long_about = "Install files into INSDESTTREE."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(short = 'r')]
    recursive: bool,

    #[arg(required = true, allow_hyphen_values = true, value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let build = get_build_mut();
    let dest = build.env(INSDESTTREE);
    let opts = &build.insopts;
    let install = build.install().dest(dest)?.file_options(opts);

    let (dirs, files): (Vec<_>, Vec<_>) = cmd.paths.into_iter().partition(|p| p.is_dir());

    if let Some(dir) = dirs.first() {
        if cmd.recursive {
            install.recursive(dirs, NO_WALKDIR_FILTER)?;
        } else {
            return Err(Error::Base(format!("installing directory without -r: {dir}")));
        }
    }

    if !files.is_empty() {
        install.files(&files)?;
    }

    Ok(ExecStatus::Success)
}

make_builtin!("doins", doins_builtin);

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs;
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;

    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, doins, insinto, insopts};

    cmd_scope_tests!("doins path/to/file");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(doins, &[0]);

        // missing args
        assert!(doins(&["-r"]).is_err());

        let _file_tree = FileTree::new();

        // non-recursive directory
        fs::create_dir("dir").unwrap();
        let r = doins(&["dir"]);
        assert_err_re!(r, "^installing directory without -r: dir$");

        // nonexistent
        let r = doins(&["nonexistent"]);
        assert_err_re!(r, "^invalid file: nonexistent: No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        fs::File::create("file").unwrap();

        // simple file
        doins(&["file"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/file"
            mode = 0o100644
        "#,
        );

        // explicit root dir
        insinto(&["/"]).unwrap();
        doins(&["file"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/file"
            mode = 0o100644
        "#,
        );

        insinto(&["-"]).unwrap();
        doins(&["file"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/-/file"
            mode = 0o100644
        "#,
        );

        for dir in ["newdir", "/newdir"] {
            // recursive using `insinto` and `insopts`
            fs::create_dir_all("dir1/subdir").unwrap();
            fs::File::create("dir1/subdir/file").unwrap();
            insinto(&[dir]).unwrap();
            insopts(&["-m0755"]).unwrap();
            doins(&["-r", "dir1"]).unwrap();
            file_tree.assert(
                r#"
                [[files]]
                path = "/newdir/dir1/subdir/file"
                mode = 0o100755
            "#,
            );

            // non-unicode nested path
            fs::create_dir_all("dir2/subdir").unwrap();
            let mut path = PathBuf::from("dir2/subdir");
            path.push(OsStr::from_bytes(&[0x66, 0x6f, 0x80, 0x6f]));
            fs::File::create(path).unwrap();
            doins(&["-r", "dir2"]).unwrap();
            file_tree.assert(
                r#"
                [[files]]
                path = "/newdir/dir2/subdir/fo�o"
                mode = 0o100755
            "#,
            );
        }
    }
}
