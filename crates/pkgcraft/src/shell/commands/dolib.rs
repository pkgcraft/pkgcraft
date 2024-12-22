use itertools::Either;
use scallop::{Error, ExecStatus};

use crate::macros::build_path;
use crate::shell::environment::Variable::DESTTREE;
use crate::shell::get_build_mut;
use crate::shell::utils::get_libdir;

use super::make_builtin;

const LONG_DOC: &str = "Install libraries.";

pub(super) fn install_lib(
    args: &[&str],
    opts: Option<&[&str]>,
) -> scallop::Result<ExecStatus> {
    let build = get_build_mut();
    let libdir = get_libdir(Some("lib")).unwrap();
    let dest = build_path!(build.env(DESTTREE), &libdir);
    let options = match opts {
        Some(vals) => Either::Left(vals.iter().copied()),
        None => Either::Right(build.libopts.iter().map(|s| s.as_str())),
    };
    build
        .install()
        .dest(dest)?
        .file_options(options)
        .files(args)?;
    Ok(ExecStatus::Success)
}

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    install_lib(args, None)
}

const USAGE: &str = "dolib path/to/lib";
make_builtin!("dolib", dolib_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use scallop::variables::{bind, unbind};

    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_args, cmd_scope_tests, dolib, into, libopts};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dolib, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = dolib(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        // force libdir default
        bind("ABI", "arch", None, None).unwrap();
        unbind("LIBDIR_arch").unwrap();

        fs::File::create("pkgcraft").unwrap();
        dolib(&["pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/lib/pkgcraft"
            mode = 0o100644
        "#,
        );

        // force libdir override
        bind("LIBDIR_arch", "lib64", None, None).unwrap();

        dolib(&["pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/lib64/pkgcraft"
            mode = 0o100644
        "#,
        );

        // custom mode and install dir
        into(&["/"]).unwrap();
        libopts(&["-m0755"]).unwrap();
        dolib(&["pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/lib64/pkgcraft"
            mode = 0o100755
        "#,
        );
    }
}
