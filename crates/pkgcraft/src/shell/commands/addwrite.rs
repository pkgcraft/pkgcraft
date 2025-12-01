use camino::Utf8PathBuf;
use scallop::ExecStatus;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "addwrite",
    disable_help_flag = true,
    long_about = "Add a path to the sandbox write list."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(allow_hyphen_values = true)]
    path: Utf8PathBuf,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let _cmd = Command::try_parse_args(args)?;
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

make_builtin!("addwrite", addwrite_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_cmd, cmd_scope_tests, functions::addwrite};

    cmd_scope_tests!("addwrite /dev");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(addwrite, &[0, 2]);
    }
}
