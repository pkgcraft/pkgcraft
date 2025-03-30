use std::io::Write;

use scallop::ExecStatus;

use crate::io::stdout;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "hasv",
    disable_help_flag = true,
    long_about = "The same as has, but also prints the first argument if found."
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
    if found {
        write!(stdout(), "{}", args[0])?;
    }

    Ok(ExecStatus::from(found))
}

make_builtin!("hasv", hasv_builtin, false);

#[cfg(test)]
mod tests {
    use crate::io::stdout;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, hasv};
    use super::*;

    cmd_scope_tests!("hasv needle ${haystack}");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(hasv, &[0]);
    }

    #[test]
    fn contains() {
        // no haystack
        assert_eq!(hasv(&["1"]).unwrap(), ExecStatus::Failure(1));
        // single element
        assert_eq!(hasv(&["1", "1"]).unwrap(), ExecStatus::Success);
        assert_eq!(stdout().get(), "1");
        // multiple elements
        assert_eq!(hasv(&["5", "1", "2", "3", "4", "5"]).unwrap(), ExecStatus::Success);
        assert_eq!(stdout().get(), "5");
        assert_eq!(hasv(&["6", "1", "2", "3", "4", "5"]).unwrap(), ExecStatus::Failure(1));
        assert_eq!(hasv(&["-a", "1", "2", "3", "4", "-a"]).unwrap(), ExecStatus::Success);
        assert_eq!(stdout().get(), "-a");
    }
}
