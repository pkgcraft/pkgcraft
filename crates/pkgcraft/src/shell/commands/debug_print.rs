use scallop::ExecStatus;
use tracing::debug;

use super::make_builtin;

const LONG_DOC: &str = "\
If in a special debug mode, the arguments should be outputted or recorded using some kind of debug
logging.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    debug!("{}", args.join(" "));
    Ok(ExecStatus::Success)
}

const USAGE: &str = "debug-print msg";
make_builtin!("debug-print", debug_print_builtin);

#[cfg(test)]
mod tests {
    use super::super::cmd_scope_tests;
    use super::*;

    cmd_scope_tests!(USAGE);

    // TODO: add usage tests
}
