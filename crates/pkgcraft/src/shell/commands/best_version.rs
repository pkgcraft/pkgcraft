use std::io::Write;

use scallop::ExecStatus;

use crate::io::stdout;

use super::_query_cmd::query_cmd;
use super::make_builtin;

const LONG_DOC: &str =
    "Output the highest matching version of a package dependency is installed.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if let Some(cpv) = query_cmd(args)?.last() {
        write!(stdout(), "{cpv}")?;
        Ok(ExecStatus::Success)
    } else {
        write!(stdout(), "")?;
        Ok(ExecStatus::Failure(1))
    }
}

make_builtin!("best_version", best_version_builtin);

#[cfg(test)]
mod tests {
    use super::super::cmd_scope_tests;

    cmd_scope_tests!("best_version cat/pkg");

    // TODO: add usage tests
}
