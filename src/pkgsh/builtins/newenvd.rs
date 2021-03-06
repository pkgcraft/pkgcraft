use scallop::builtins::ExecStatus;
use scallop::Result;

use super::_new::new;
use super::doenvd::run as doenvd;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed environment files into /etc/env.d/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, doenvd)
}

const USAGE: &str = "newenvd path/to/env_file new_filename";
make_builtin!("newenvd", newenvd_builtin, run, LONG_DOC, USAGE, &[("0-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::write_stdin;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as newenvd;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(newenvd, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("env").unwrap();
        newenvd(&["env", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/etc/env.d/pkgcraft"
        "#,
        );

        // re-run using data from stdin
        write_stdin!("pkgcraft");
        newenvd(&["-", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/etc/env.d/pkgcraft"
            data = "pkgcraft"
        "#,
        );
    }
}
