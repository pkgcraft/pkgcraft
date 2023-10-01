use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::SrcInstall;

use super::make_builtin;

const LONG_DOC: &str = "Install executables.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let build = get_build_mut();
    let dest = &build.exedesttree;
    let opts = &build.exeopts;
    let install = build.install().dest(dest)?.file_options(opts);
    install.files(args)?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "doexe path/to/executable";
make_builtin!("doexe", doexe_builtin, run, LONG_DOC, USAGE, [("..", [SrcInstall])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::macros::assert_err_re;
    use crate::shell::test::FileTree;

    use super::super::exeinto::run as exeinto;
    use super::super::exeopts::run as exeopts;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as doexe;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(doexe, &[0]);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = doexe(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        let default_mode = 0o100755;
        let custom_mode = 0o100777;

        fs::File::create("pkgcraft").unwrap();
        doexe(&["pkgcraft"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/pkgcraft"
            mode = {default_mode}
        "#
        ));

        // custom mode and install dir
        for dir in ["/opt/bin", "opt/bin"] {
            exeinto(&[dir]).unwrap();
            exeopts(&["-m0777"]).unwrap();
            doexe(&["pkgcraft"]).unwrap();
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/opt/bin/pkgcraft"
                mode = {custom_mode}
            "#
            ));
        }
    }
}
