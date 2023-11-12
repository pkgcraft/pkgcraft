use scallop::ExecStatus;

use super::make_builtin;

const LONG_DOC: &str = "Add a directory to the sandbox permitted write list.";

#[doc = stringify!(LONG_DOC)]
fn run(_args: &[&str]) -> scallop::Result<ExecStatus> {
    // TODO: fill out this stub
    Ok(ExecStatus::Success)
}

const USAGE: &str = "addwrite /dev";
make_builtin!("addwrite", addwrite_builtin);

#[cfg(test)]
mod tests {
    use super::super::cmd_scope_tests;
    use super::*;

    cmd_scope_tests!(USAGE);
}
