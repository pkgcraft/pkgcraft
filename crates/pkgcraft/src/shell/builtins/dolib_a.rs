use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::shell::phase::PhaseKind::SrcInstall;

use super::dolib::install_lib;
use super::{make_builtin, Scopes::Phase};

const LONG_DOC: &str = "Install static libraries.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    install_lib(args, Some(&["-m0644"]))
}

const USAGE: &str = "dolib.a path/to/lib.a";
make_builtin!("dolib.a", dolib_a_builtin, run, LONG_DOC, USAGE, &[("..", &[Phase(SrcInstall)])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::Config;
    use crate::shell::test::FileTree;
    use crate::shell::BuildData;

    use super::super::into::run as into;
    use super::super::libopts::run as libopts;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as dolib_a;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dolib_a, &[0]);
    }

    #[test]
    fn creation() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let raw_pkg = t.create_ebuild("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        let file_tree = FileTree::new();
        let default_mode = 0o100644;

        fs::File::create("pkgcraft.a").unwrap();
        dolib_a(&["pkgcraft.a"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/usr/lib/pkgcraft.a"
            mode = {default_mode}
        "#
        ));

        // custom install dir with libopts ignored
        into(&["/"]).unwrap();
        libopts(&["-m0755"]).unwrap();
        dolib_a(&["pkgcraft.a"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/lib/pkgcraft.a"
            mode = {default_mode}
        "#
        ));
    }
}
