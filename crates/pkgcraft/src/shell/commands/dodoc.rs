use std::path::Path;

use scallop::{Error, ExecStatus};

use crate::files::NO_WALKDIR_FILTER;
use crate::macros::build_path;
use crate::shell::environment::Variable::DOCDESTTREE;
use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Install documentation files.";

/// Install document files from a given list of paths.
pub(crate) fn install_docs<P: AsRef<Path>>(
    recursive: bool,
    paths: &[P],
    dest: &str,
) -> scallop::Result<ExecStatus> {
    let build = get_build_mut();
    let dest = build_path!("/usr/share/doc", build.cpv().pf(), dest.trim_start_matches('/'));
    let install = build.install().dest(dest)?;

    let (dirs, files): (Vec<_>, Vec<_>) =
        paths.iter().map(|p| p.as_ref()).partition(|p| p.is_dir());

    if !dirs.is_empty() {
        if recursive {
            install.recursive(dirs, NO_WALKDIR_FILTER)?;
        } else {
            let dir = dirs[0].to_string_lossy();
            return Err(Error::Base(format!("installing directory without -r: {dir}")));
        }
    }

    install.files(files)?;

    Ok(ExecStatus::Success)
}

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let (recursive, args) = match args {
        ["-r", args @ ..] => (true, args),
        _ => (false, args),
    };

    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".to_string()));
    }

    let dest = get_build_mut().env(DOCDESTTREE);
    install_docs(recursive, args, dest)
}

const USAGE: &str = "dodoc doc_file";
make_builtin!("dodoc", dodoc_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::test::FileTree;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_args, cmd_scope_tests, docinto, dodoc};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dodoc, &[0]);

        // missing args
        let r = dodoc(&["-r"]);
        assert_err_re!(r, "^requires 1 or more args, got 0");

        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        let _file_tree = FileTree::new();

        // non-recursive directory
        fs::create_dir("dir").unwrap();
        let r = dodoc(&["dir"]);
        assert_err_re!(r, "^installing directory without -r: dir$");

        // nonexistent
        let r = dodoc(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();

        // simple file
        fs::File::create("file").unwrap();
        dodoc(&["file"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/file"
            mode = 0o100644
        "#,
        );

        // recursive using `docinto`
        fs::create_dir_all("doc/subdir").unwrap();
        fs::File::create("doc/subdir/file").unwrap();
        docinto(&["newdir"]).unwrap();
        dodoc(&["-r", "doc"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/newdir/doc/subdir/file"
        "#,
        );

        // handling for paths ending in '/.'
        docinto(&["/newdir"]).unwrap();
        dodoc(&["-r", "doc/."]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/newdir/subdir/file"
        "#,
        );
    }
}
