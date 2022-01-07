use std::sync::atomic::Ordering;

use scallop::builtins::Builtin;
use scallop::command::Command;
use scallop::{Error, Result};

use super::NONFATAL;

static LONG_DOC: &str = "\
Takes one or more arguments and executes them as a command, preserving the exit status. If this
results in a command being called that would normally abort the build process due to a failure,
instead a non-zero exit status shall be returned.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<i32> {
    if args.is_empty() {
        return Err(Error::new("no arguments specified"));
    }

    NONFATAL.store(true, Ordering::Relaxed);
    let orig_cmd = args.join(" ");
    let cmd = Command::new(orig_cmd, None)?;
    cmd.execute().ok();
    NONFATAL.store(false, Ordering::Relaxed);

    Ok(0)
}

pub static BUILTIN: Builtin = Builtin {
    name: "nonfatal",
    func: run,
    help: LONG_DOC,
    usage: "nonfatal cmd arg1 arg2",
    exit_on_error: false,
};
