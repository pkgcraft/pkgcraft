use scallop::ExecStatus;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "has",
    disable_help_flag = true,
    long_about = indoc::indoc! {"
        Returns success if the first argument is found in subsequent arguments, failure
        otherwise.
    "}
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

make_builtin!("has", has_builtin, false);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_cmd, cmd_scope_tests, has};
    use super::*;

    cmd_scope_tests!("has needle ${haystack}");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(has, &[0]);
    }

    #[test]
    fn contains() {
        // no haystack
        assert_eq!(has(&["1"]).unwrap(), ExecStatus::Failure(1));
        // single element
        assert_eq!(has(&["1", "1"]).unwrap(), ExecStatus::Success);
        // multiple elements
        assert_eq!(has(&["5", "1", "2", "3", "4", "5"]).unwrap(), ExecStatus::Success);
        assert_eq!(has(&["6", "1", "2", "3", "4", "5"]).unwrap(), ExecStatus::Failure(1));
        assert_eq!(has(&["-", "1", "2", "3", "4", "-"]).unwrap(), ExecStatus::Success);
    }
}
