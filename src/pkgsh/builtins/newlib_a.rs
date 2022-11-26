use scallop::builtins::ExecStatus;

use super::_new::new;
use super::dolib_a::run as dolib_a;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed static libraries.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, dolib_a)
}

const USAGE: &str = "newlib.a path/to/lib.a new_filename";
make_builtin!("newlib.a", newlib_a_builtin, run, LONG_DOC, USAGE, &[("0-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use crate::config::Config;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::{write_stdin, BuildData};

    use super::super::into::run as into;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as newlib_a;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(newlib_a, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();
        let (_, cpv) = t.create_ebuild("cat/pkg-1", []).unwrap();
        BuildData::update(&cpv, &repo);

        let file_tree = FileTree::new();

        fs::File::create("lib").unwrap();
        newlib_a(&["lib", "pkgcraft.a"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/lib/pkgcraft.a"
        "#,
        );

        // custom install dir using data from stdin
        write_stdin!("pkgcraft");
        into(&["/"]).unwrap();
        newlib_a(&["-", "pkgcraft.a"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/lib/pkgcraft.a"
            data = "pkgcraft"
        "#,
        );
    }
}
