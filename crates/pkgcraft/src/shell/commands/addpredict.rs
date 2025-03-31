use camino::Utf8PathBuf;
use scallop::ExecStatus;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "addpredict",
    disable_help_flag = true,
    long_about = indoc::indoc! {"
        Add a path to the predict list. Any write to a location in this list will be
        denied, but will not trigger access violation messages or abort the build process.
    "}
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(allow_hyphen_values = true)]
    path: Utf8PathBuf,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let _cmd = Command::try_parse_args(args)?;
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

make_builtin!("addpredict", addpredict_builtin, true);

#[cfg(test)]
mod tests {
    use super::super::{addpredict, assert_invalid_cmd, cmd_scope_tests};

    cmd_scope_tests!("addpredict /proc");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(addpredict, &[0, 2]);
    }
}
