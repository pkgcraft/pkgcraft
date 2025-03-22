use scallop::ExecStatus;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(name = "hasq", long_about = "Deprecated synonym for has.")]
struct Command {
    needle: String,
    haystack: Vec<String>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let found = cmd.haystack.contains(&cmd.needle);
    Ok(ExecStatus::from(found))
}

make_builtin!("hasq", hasq_builtin);

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
    }
}
