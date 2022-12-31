use std::time::{Duration, Instant};

use crate::builtins::{make_builtin, ExecStatus, ScopedOptions};
use crate::command::Command;
use crate::Error;

static LONG_DOC: &str = "Profile a given function or command.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> crate::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    // Disable shell history if history support is enabled, so the command being profiled isn't
    // added, reverting to the previous state on scope exit.
    let mut opts = ScopedOptions::default();
    let _ = opts.disable(["history"]);

    let mut loops = 0;
    let mut elapsed = Duration::new(0, 0);
    let orig_cmd = args.join(" ");
    eprintln!("profiling: {orig_cmd}");

    // force success so the shell doesn't exit prematurely while profiling
    let cmd_str = format!("{orig_cmd} || :");
    let cmd = Command::new(cmd_str, None)?;
    let start = Instant::now();
    while elapsed.as_secs() < 3 {
        cmd.execute().ok();
        loops += 1;
        elapsed = start.elapsed();
    }

    let per_loop = elapsed / loops;
    eprintln!("elapsed {elapsed:?}, loops: {loops}, per loop: {per_loop:?}");
    Ok(ExecStatus::Success)
}

make_builtin!("profile", profile_builtin, run, LONG_DOC, "profile func arg1 arg2");
