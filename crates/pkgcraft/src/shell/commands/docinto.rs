use scallop::{Error, ExecStatus};

use crate::shell::environment::Variable::DOCDESTTREE;
use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "\
Takes exactly one argument and sets the install path for dodoc and other doc-related commands.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let [path] = args else {
        return Err(Error::Base(format!("requires 1 arg, got {}", args.len())));
    };

    get_build_mut().override_var(DOCDESTTREE, path)?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "docinto /install/path";
make_builtin!("docinto", docinto_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::test::FileTree;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::TEST_DATA;

    use super::super::{assert_invalid_args, cmd_scope_tests, docinto, dodoc};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(docinto, &[0, 2]);
    }

    #[test]
    fn creation() {
        let repo = TEST_DATA.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();
        fs::File::create("file").unwrap();

        docinto(&["examples"]).unwrap();
        dodoc(&["file"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/examples/file"
        "#,
        );

        docinto(&["/"]).unwrap();
        dodoc(&["file"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/file"
        "#,
        );
    }
}
