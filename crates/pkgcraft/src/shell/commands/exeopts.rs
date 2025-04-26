use scallop::ExecStatus;

use crate::shell::get_build_mut;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "exeopts",
    disable_help_flag = true,
    long_about = "Sets the options for installing executables via `doexe` and similar commands."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(required = true, allow_hyphen_values = true, value_name = "OPTION")]
    options: Vec<String>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    get_build_mut().exeopts = cmd.options.into_iter().collect();
    Ok(ExecStatus::Success)
}

make_builtin!("exeopts", exeopts_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, doexe, exeopts};

    cmd_scope_tests!("exeopts -m0755");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(exeopts, &[0]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        fs::File::create("pkgcraft").unwrap();

        exeopts(&["-m0777"]).unwrap();
        doexe(&["pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/pkgcraft"
            mode = 0o100777
        "#,
        );
    }
}
