use scallop::ExecStatus;

use crate::shell::environment::Variable::EXEDESTTREE;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "exeinto",
    disable_help_flag = true,
    long_about = "Takes exactly one argument and sets the install path for doexe and newexe."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(allow_hyphen_values = true)]
    path: String,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    EXEDESTTREE.set(cmd.path)?;
    Ok(ExecStatus::Success)
}

make_builtin!("exeinto", exeinto_builtin, true);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, doexe, exeinto};

    cmd_scope_tests!("exeinto /install/path");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(exeinto, &[0, 2]);

        // -- signifies an end of options
        assert!(exeinto(&["--"]).is_err());
        assert!(exeinto(&["--", "--"]).is_ok());
    }

    #[test]
    fn set_path() {
        let file_tree = FileTree::new();
        fs::File::create("file").unwrap();

        exeinto(&["/test/path"]).unwrap();
        doexe(&["file"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/test/path/file"
            mode = 0o100755
        "#,
        );

        exeinto(&["-"]).unwrap();
        doexe(&["file"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/-/file"
            mode = 0o100755
        "#,
        );
    }
}
