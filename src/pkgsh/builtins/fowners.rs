use std::path::Path;
use std::process::Command;

use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::command::RunCommand;
use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "Run `chown` taking paths relative to the image directory.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.len() < 2 {
        return Err(Error::Builtin(format!("requires at least 2 args, got {}", args.len())));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let env = &d.borrow().env;
        let destdir = Path::new(
            env.get("ED")
                .unwrap_or_else(|| env.get("D").expect("$D undefined")),
        );

        let mut chown = Command::new("chown");
        for arg in args {
            let path = Path::new(arg.trim_start_matches('/'));
            chown.arg(destdir.join(path));
        }

        chown.run()?;

        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "fowners user:group /path/to/file";
make_builtin!(
    "fowners",
    fowners_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("0-", &["src_install", "pkg_preinst", "pkg_postinst"])]
);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as fowners;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(fowners, &[0, 1]);
    }

    // TODO: add usage tests
}
