use scallop::command::Command;
use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "\
Takes one or more arguments and executes them as a command, preserving the exit status. If this
results in a command being called that would normally abort the build process due to a failure,
instead a non-zero exit status shall be returned.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    // enable nonfatal status
    let build = get_build_mut();
    build.nonfatal = true;

    // run the specified command
    let cmd = Command::new(args.join(" "), None)?;
    let status = match cmd.execute() {
        Ok(s) => s,
        Err(Error::Status(s)) => s,
        _ => ExecStatus::Failure(1),
    };

    // disable nonfatal status
    build.nonfatal = false;
    Ok(status)
}

const USAGE: &str = "nonfatal cmd arg1 arg2";
make_builtin!("nonfatal", nonfatal_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests, nonfatal};
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(nonfatal, &[0]);
    }

    #[test]
    fn exit_status() {
        let status = nonfatal(&["nonexistent_cmd"]).unwrap();
        assert!(i32::from(status) != 0);
    }

    #[test]
    fn die() {
        let status = nonfatal(&["die", "-n", "message"]).unwrap();
        assert!(i32::from(status) != 0);
    }

    #[test]
    fn invalid_builtin_scope() {
        let status = nonfatal(&["ewarn", "message"]).unwrap();
        assert!(i32::from(status) != 0);
    }
}
