use camino::Utf8PathBuf;
use scallop::ExecStatus;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(name = "adddeny", long_about = "Add a path to the sandbox deny list.")]
struct Command {
    path: Utf8PathBuf,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let _cmd = Command::try_parse_args(args)?;
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

make_builtin!("adddeny", adddeny_builtin);

#[cfg(test)]
mod tests {
    use super::super::{adddeny, assert_invalid_cmd, cmd_scope_tests};

    cmd_scope_tests!("adddeny /path/to/deny");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(adddeny, &[0, 2]);
    }
}
