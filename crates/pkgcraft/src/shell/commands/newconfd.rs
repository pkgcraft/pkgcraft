use scallop::ExecStatus;

use super::_new::new;
use super::doconfd;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed config files into /etc/conf.d/.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, doconfd)
}

make_builtin!("newconfd", newconfd_builtin, true);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::io::stdin;
    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_args, cmd_scope_tests, newconfd};
    use super::*;

    cmd_scope_tests!("newconfd path/to/config/file new_filename");

    #[test]
    fn invalid_args() {
        assert_invalid_args(newconfd, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("config").unwrap();
        newconfd(&["config", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/etc/conf.d/pkgcraft"
            mode = 0o100644
        "#,
        );

        // re-run using data from stdin
        stdin().inject("pkgcraft").unwrap();
        newconfd(&["-", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/etc/conf.d/pkgcraft"
            data = "pkgcraft"
            mode = 0o100644
        "#,
        );
    }
}
