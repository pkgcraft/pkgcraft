use scallop::ExecStatus;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "hasq",
    disable_help_flag = true,
    long_about = "Deprecated synonym for has."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(allow_hyphen_values = true)]
    needle: String,

    #[arg(allow_hyphen_values = true)]
    haystack: Vec<String>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let found = cmd.haystack.contains(&cmd.needle);
    Ok(ExecStatus::from(found))
}

make_builtin!("hasq", hasq_builtin, true);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_cmd, cmd_scope_tests, hasq};
    use super::*;

    cmd_scope_tests!("hasq needle ${haystack}");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(hasq, &[0]);
    }

    #[test]
    fn contains() {
        // no haystack
        assert_eq!(hasq(&["1"]).unwrap(), ExecStatus::Failure(1));
        // single element
        assert_eq!(hasq(&["1", "1"]).unwrap(), ExecStatus::Success);
        // multiple elements
        assert_eq!(hasq(&["5", "1", "2", "3", "4", "5"]).unwrap(), ExecStatus::Success);
        assert_eq!(hasq(&["6", "1", "2", "3", "4", "5"]).unwrap(), ExecStatus::Failure(1));
        assert_eq!(hasq(&["-", "1", "2", "3", "4", "-"]).unwrap(), ExecStatus::Success);
    }
}
