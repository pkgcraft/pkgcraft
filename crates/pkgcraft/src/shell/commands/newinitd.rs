use scallop::ExecStatus;

use super::_new::new;
use super::doinitd;
use super::make_builtin;

// TODO: convert to clap parser
//const LONG_DOC: &str = "Install renamed init scripts into /etc/init.d/.";

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    new(args, doinitd)
}

make_builtin!("newinitd", newinitd_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::io::stdin;
    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_args, cmd_scope_tests, newinitd};

    cmd_scope_tests!("newinitd path/to/init/file new_filename");

    #[test]
    fn invalid_args() {
        assert_invalid_args(newinitd, &[0, 1, 3]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        fs::File::create("init").unwrap();
        newinitd(&["init", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/etc/init.d/pkgcraft"
            mode = 0o100755
        "#,
        );

        // re-run using data from stdin
        stdin().inject("pkgcraft").unwrap();
        newinitd(&["-", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/etc/init.d/pkgcraft"
            data = "pkgcraft"
            mode = 0o100755
        "#,
        );
    }
}
