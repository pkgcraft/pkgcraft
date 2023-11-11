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
    use crate::config::Config;
    use crate::shell::{assert_stderr, assert_stdout, BuildData};

    use super::super::{assert_invalid_args, builtin_scope_tests, nonfatal};
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(nonfatal, &[0]);
    }

    #[test]
    fn success() {
        let mut config = Config::default();
        let t = config.temp_repo("test1", 0, None).unwrap();
        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        let status = nonfatal(&["ver_cut", "2-3", "1.2.3"]).unwrap();
        assert_stdout!("2.3");
        assert!(i32::from(status) == 0);
    }

    #[test]
    fn nonexistent_cmd() {
        let status = nonfatal(&["nonexistent_cmd"]).unwrap();
        assert!(i32::from(status) != 0);
    }

    #[test]
    fn die() {
        let status = nonfatal(&["die", "-n", "message"]).unwrap();
        assert_stderr!("message\n");
        assert!(i32::from(status) != 0);
    }

    #[test]
    fn invalid_builtin_scope() {
        let status = nonfatal(&["ewarn", "message"]).unwrap();
        assert!(i32::from(status) != 0);
    }
}
