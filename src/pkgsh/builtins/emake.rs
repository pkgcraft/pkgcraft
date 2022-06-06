use std::io::Write;
use std::process::Command;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::variables::{string_value, string_vec};
use scallop::{Error, Result};

use super::{PkgBuiltin, PHASE};
use crate::command::RunCommand;
use crate::pkgsh::utils::makefile_exists;
use crate::pkgsh::write_stdout;

const LONG_DOC: &str = "Run the make command for a package.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if !makefile_exists() {
        return Err(Error::Builtin("nonexistent makefile".into()));
    }

    let make_prog = string_value("MAKE").unwrap_or_else(|| String::from("make"));
    let mut emake = Command::new(make_prog);
    if let Ok(opts) = string_vec("MAKEOPTS") {
        emake.args(&opts);
    }

    emake.args(args);
    write_stdout!("{}", emake.to_vec().join(" "));
    emake.run()?;
    Ok(ExecStatus::Success)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "emake",
            func: run,
            help: LONG_DOC,
            usage: "emake -C builddir",
        },
        &[("0-", &[PHASE])],
    )
});

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs::File;

    use rusty_fork::rusty_fork_test;
    use scallop::shell::Shell;
    use scallop::variables::bind;
    use tempfile::tempdir;

    use super::run as emake;
    use crate::command::last_command;
    use crate::macros::assert_err_re;

    rusty_fork_test! {
        #[test]
        fn nonexistent() {
            assert_err_re!(emake(&[]), "^nonexistent makefile$");
        }

        #[test]
        fn command() {
            let _sh = Shell::new("sh");
            let dir = tempdir().unwrap();
            let makefile = dir.path().join("makefile");
            File::create(makefile).unwrap();
            env::set_current_dir(&dir).unwrap();

            // default make prog
            emake(&[]).unwrap();
            let cmd = last_command().unwrap();
            assert_eq!(cmd[0], "make");

            // custom args
            let args = ["-C", "build", "install"];
            emake(&args).unwrap();
            let cmd = last_command().unwrap();
            assert_eq!(cmd[1..], args);

            // using $MAKEOPTS settings
            bind("MAKEOPTS", "-j10", None, None).unwrap();
            emake(&[]).unwrap();
            let cmd = last_command().unwrap();
            assert_eq!(cmd[1..], ["-j10"]);
            bind("MAKEOPTS", "-j20 -l 20", None, None).unwrap();
            emake(&[]).unwrap();
            let cmd = last_command().unwrap();
            assert_eq!(cmd[1..], ["-j20", "-l", "20"]);

            // custom $MAKE prog
            bind("MAKE", "custom-make", None, None).unwrap();
            emake(&[]).unwrap();
            let cmd = last_command().unwrap();
            assert_eq!(cmd[0], "custom-make");
        }
    }
}
