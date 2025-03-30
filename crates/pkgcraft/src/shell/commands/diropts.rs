use scallop::ExecStatus;

use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "diropts",
    disable_help_flag = true,
    long_about = "Sets the options for directory creation via `dodir` and similar commands."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(required = true, allow_hyphen_values = true, value_name = "OPTION")]
    options: Vec<String>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    get_build_mut().diropts = cmd.options.into_iter().collect();
    Ok(ExecStatus::Success)
}

make_builtin!("diropts", diropts_builtin, true);

#[cfg(test)]
mod tests {
    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, diropts, dodir};

    cmd_scope_tests!("diropts -m0750");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(diropts, &[0]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();

        // change mode and re-run dodir()
        diropts(&["-m0777"]).unwrap();
        dodir(&["dir"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/dir"
            mode = 0o40777
        "#,
        );
    }
}
