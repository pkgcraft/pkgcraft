use scallop::ExecStatus;

use super::_query_cmd::query_cmd;
use super::make_builtin;

const LONG_DOC: &str = "Determine if a package dependency is installed.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cpvs = query_cmd(args)?;
    Ok((!cpvs.is_empty()).into())
}

const USAGE: &str = "has_version 'cat/pkg[use]'";
make_builtin!("has_version", has_version_builtin, false);

#[cfg(test)]
mod tests {
    use super::super::cmd_scope_tests;
    use super::*;

    cmd_scope_tests!(USAGE);

    // TODO: add usage tests
}
