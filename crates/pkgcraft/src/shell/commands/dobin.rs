use nix::unistd::geteuid;
use scallop::{Error, ExecStatus};

use crate::macros::build_path;
use crate::shell::environment::Variable::DESTTREE;
use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Install executables into DESTTREE/bin.";

pub(super) fn install_bin(args: &[&str], dest: &str) -> scallop::Result<ExecStatus> {
    let build = get_build_mut();
    let dest = build_path!(build.env(DESTTREE), dest);
    let opts: &[&str] = if geteuid().is_root() {
        &["-m0755", "-o", "root", "-g", "root"]
    } else {
        &["-m0755"]
    };
    build.install().dest(dest)?.file_options(opts).files(args)?;
    Ok(ExecStatus::Success)
}

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    install_bin(args, "bin")
}

const USAGE: &str = "dobin path/to/executable";
make_builtin!("dobin", dobin_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_args, cmd_scope_tests, dobin, exeopts, into};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dobin, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = dobin(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("pkgcraft").unwrap();
        dobin(&["pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/bin/pkgcraft"
            mode = 0o100755
        "#,
        );

        // custom install dir with libopts ignored
        into(&["/"]).unwrap();
        exeopts(&["-m0777"]).unwrap();
        dobin(&["pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/bin/pkgcraft"
            mode = 0o100755
        "#,
        );
    }
}
