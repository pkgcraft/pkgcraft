use scallop::ExecStatus;

use super::make_builtin;

const LONG_DOC: &str = "Add a directory to the sandbox predict list.";

#[doc = stringify!(LONG_DOC)]
fn run(_args: &[&str]) -> scallop::Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

const USAGE: &str = "addpredict /proc";
make_builtin!("addpredict", addpredict_builtin);

#[cfg(test)]
mod tests {
    use super::super::builtin_scope_tests;
    use super::*;

    builtin_scope_tests!(USAGE);
}
