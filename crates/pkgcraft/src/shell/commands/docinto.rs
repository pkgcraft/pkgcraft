use scallop::ExecStatus;

use crate::shell::environment::Variable::DOCDESTTREE;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "docinto",
    long_about = indoc::indoc! {"
        Takes exactly one argument and sets the install path for dodoc and other
        doc-related commands.
    "}
)]
struct Command {
    path: String,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    DOCDESTTREE.set(cmd.path)?;
    Ok(ExecStatus::Success)
}

const USAGE: &str = "docinto /install/path";
make_builtin!("docinto", docinto_builtin, true);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::test::FileTree;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, docinto, dodoc};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(docinto, &[0, 2]);
    }

    #[test]
    fn creation() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
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
