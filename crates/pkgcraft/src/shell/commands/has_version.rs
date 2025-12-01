use scallop::ExecStatus;

use super::_query_cmd::query_cmd;
use super::make_builtin;

// TODO: convert to clap parser
//const LONG_DOC: &str = "Determine if a package dependency is installed.";

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cpvs = query_cmd(args)?;
    Ok((!cpvs.is_empty()).into())
}

make_builtin!("has_version", has_version_builtin);

#[cfg(test)]
mod tests {
    use super::super::cmd_scope_tests;

    cmd_scope_tests!("has_version 'cat/pkg[use]'");

    // TODO: add usage tests
}
