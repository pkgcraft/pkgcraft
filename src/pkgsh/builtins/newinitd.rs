use scallop::builtins::ExecStatus;
use scallop::Result;

use super::_new::new;
use super::doinitd::run as doinitd;
use super::make_builtin;

const LONG_DOC: &str = "Install renamed init scripts into /etc/init.d/.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, doinitd)
}

const USAGE: &str = "newinitd path/to/init/file new_filename";
make_builtin!("newinitd", newinitd_builtin, run, LONG_DOC, USAGE, &[("0-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::write_stdin;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as newinitd;
    use super::*;

    builtin_scope_tests!(USAGE);

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
        "#,
        );

        // re-run using data from stdin
        write_stdin!("pkgcraft");
        newinitd(&["-", "pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/etc/init.d/pkgcraft"
            data = "pkgcraft"
        "#,
        );
    }
}
