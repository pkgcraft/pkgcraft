use std::path::Path;

use scallop::{Error, ExecStatus};

use crate::files::NO_WALKDIR_FILTER;
use crate::macros::build_from_paths;
use crate::pkg::Package;
use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::SrcInstall;

use super::make_builtin;

const LONG_DOC: &str = "Install documentation files.";

/// Install document files from a given list of paths.
pub(crate) fn install_docs<P: AsRef<Path>>(
    recursive: bool,
    paths: &[P],
) -> scallop::Result<ExecStatus> {
    let build = get_build_mut();
    let dest = build_from_paths!(
        "/usr/share/doc",
        build.pkg()?.cpv().pf(),
        build.docdesttree.trim_start_matches('/')
    );
    let install = build.install().dest(dest)?;

    let (dirs, files): (Vec<_>, Vec<_>) =
        paths.iter().map(|p| p.as_ref()).partition(|p| p.is_dir());

    if !dirs.is_empty() {
        if recursive {
            install.recursive(dirs, NO_WALKDIR_FILTER)?;
        } else {
            return Err(Error::Base(format!("non-recursive dir install: {:?}", dirs[0])));
        }
    }

    install.files(files)?;

    Ok(ExecStatus::Success)
}

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let (recursive, args) = match args {
        ["-r", args @ ..] => (true, args),
        _ => (false, args),
    };

    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".to_string()));
    }

    install_docs(recursive, args)
}

const USAGE: &str = "dodoc doc_file";
make_builtin!("dodoc", dodoc_builtin, run, LONG_DOC, USAGE, [("..", [SrcInstall])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::shell::test::FileTree;
    use crate::shell::BuildData;

    use super::super::docinto::run as docinto;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as dodoc;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dodoc, &[0]);

        // missing args
        let r = dodoc(&["-r"]);
        assert_err_re!(r, "^requires 1 or more args, got 0");

        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_pkg(&pkg);
        let _file_tree = FileTree::new();

        // non-recursive directory
        fs::create_dir("dir").unwrap();
        let r = dodoc(&["dir"]);
        assert_err_re!(r, "^non-recursive dir install: .*$");

        // nonexistent
        let r = dodoc(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
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
