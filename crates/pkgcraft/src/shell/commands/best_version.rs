use scallop::ExecStatus;

use crate::shell::write_stdout;

use super::_query_cmd::query_cmd;
use super::make_builtin;

const LONG_DOC: &str = "Output the highest matching version of a package dependency is installed.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let mut cpvs = query_cmd(args)?;
    cpvs.sort();

    if let Some(cpv) = cpvs.last() {
        write_stdout!("{cpv}")?;
        Ok(ExecStatus::Success)
    } else {
        write_stdout!("")?;
        Ok(ExecStatus::Failure(1))
    }
}

const USAGE: &str = "best_version cat/pkg";
make_builtin!("best_version", best_version_builtin);

#[cfg(test)]
mod tests {
    use super::super::cmd_scope_tests;
    use super::*;

    cmd_scope_tests!(USAGE);

    // TODO: add usage tests
}
