use scallop::ExecStatus;

use super::_query_cmd::query_cmd;
use super::make_builtin;

const LONG_DOC: &str = "Determine if a package dependency is installed.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let mut cpvs = query_cmd(args)?;
    if cpvs.next().is_some() {
        Ok(ExecStatus::Success)
    } else {
        Ok(ExecStatus::Failure(1))
    }
}

const USAGE: &str = "has_version 'cat/pkg[use]'";
make_builtin!("has_version", has_version_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, cmd_scope_tests, has_version};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(has_version, &[0]);
    }

    // TODO: add usage tests
}
