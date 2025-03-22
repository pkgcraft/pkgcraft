use camino::Utf8PathBuf;
use scallop::ExecStatus;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(name = "adddeny", long_about = "Add a path to the sandbox write list.")]
struct Command {
    path: Utf8PathBuf,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let _cmd = Command::try_parse_args(args)?;
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

make_builtin!("addwrite", addwrite_builtin);

#[cfg(test)]
mod tests {
    use super::super::{addwrite, assert_invalid_cmd, cmd_scope_tests};

    cmd_scope_tests!("addwrite /dev");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(addwrite, &[0, 2]);
    }
}
