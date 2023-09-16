use std::process::Command;

use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::command::RunCommand;
use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::{PkgPostinst, PkgPreinst, SrcInstall};

use super::make_builtin;

const LONG_DOC: &str = "Run `chmod` taking paths relative to the image directory.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.len() < 2 {
        return Err(Error::Base(format!("requires at least 2 args, got {}", args.len())));
    }

    Command::new("chmod")
        .args(args.iter().map(|s| s.trim_start_matches('/')))
        .current_dir(get_build_mut().destdir())
        .run()?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "fperms mode /path/to/file";
make_builtin!(
    "fperms",
    fperms_builtin,
    run,
    LONG_DOC,
    USAGE,
    [("..", [SrcInstall, PkgPreinst, PkgPostinst])]
);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as fperms;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(fperms, &[0, 1]);
    }

    // TODO: add usage tests
}
