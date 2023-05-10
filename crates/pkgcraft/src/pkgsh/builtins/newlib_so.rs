use scallop::builtins::ExecStatus;

use super::_new::new;
use super::dolib_so::run as dolib_so;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed shared libraries.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, dolib_so)
}

const USAGE: &str = "newlib.so path/to/lib.so new_filename";
make_builtin!("newlib.so", newlib_so_builtin, run, LONG_DOC, USAGE, &[("..", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use crate::config::Config;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::{write_stdin, BuildData};

    use super::super::into::run as into;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as newlib_so;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(newlib_so, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();
        let (_, cpv) = t.create_ebuild("cat/pkg-1", []).unwrap();
        BuildData::update(&cpv, &repo, None);

        let file_tree = FileTree::new();

        fs::File::create("lib").unwrap();
        newlib_so(&["lib", "pkgcraft.so"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/lib/pkgcraft.so"
        "#,
        );

        // custom install dir using data from stdin
        write_stdin!("pkgcraft");
        into(&["/"]).unwrap();
        newlib_so(&["-", "pkgcraft.so"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/lib/pkgcraft.so"
            data = "pkgcraft"
        "#,
        );
    }
}
