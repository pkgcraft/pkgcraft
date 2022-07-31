use std::path::Path;
use std::process::Command;

use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::command::RunCommand;
use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "Run `chmod` taking paths relative to the image directory.";

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

        let mut chmod = Command::new("chmod");
        for arg in args {
            let path = Path::new(arg.trim_start_matches('/'));
            chmod.arg(destdir.join(path));
        }

        chmod.run()?;

        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "fperms mode /path/to/file";
make_builtin!(
    "fperms",
    fperms_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("0-", &["src_install", "pkg_preinst", "pkg_postinst"])]
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
