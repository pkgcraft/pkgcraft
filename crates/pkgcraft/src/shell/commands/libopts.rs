use scallop::ExecStatus;

use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "libopts",
    long_about = "Sets the options for installing libraries via `dolib` and similar commands."
)]
struct Command {
    #[arg(required = true, allow_hyphen_values = true, value_name = "OPTION")]
    options: Vec<String>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    get_build_mut().libopts = cmd.options.into_iter().collect();
    Ok(ExecStatus::Success)
}

const USAGE: &str = "libopts -m0644";
make_builtin!("libopts", libopts_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, dolib, libopts};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(libopts, &[0]);
    }

    #[test]
    fn creation() {
        let file_tree = FileTree::new();
        fs::File::create("pkgcraft").unwrap();

        libopts(&["-m0755"]).unwrap();
        dolib(&["pkgcraft"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/lib/pkgcraft"
            mode = 0o100755
        "#,
        );
    }
}
