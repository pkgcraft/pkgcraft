use std::io::Write;
use std::process::Command;

use scallop::builtins::ExecStatus;
use scallop::variables::{self, string_vec};
use scallop::{Error, Result};

use crate::command::RunCommand;
use crate::pkgsh::utils::makefile_exists;
use crate::pkgsh::write_stdout;

use super::{make_builtin, PHASE};

const LONG_DOC: &str = "Run the make command for a package.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if !makefile_exists() {
        return Err(Error::Base("nonexistent makefile".into()));
    }

    let make_prog = variables::optional("MAKE");
    let make_prog = make_prog.as_deref().unwrap_or("make");
    let mut emake = Command::new(make_prog);
    if let Ok(opts) = string_vec("MAKEOPTS") {
        emake.args(&opts);
    }

    emake.args(args);
    write_stdout!("{}", emake.to_vec().join(" "));
    emake.run()?;
    Ok(ExecStatus::Success)
}

const USAGE: &str = "emake -C builddir";
make_builtin!("emake", emake_builtin, run, LONG_DOC, USAGE, &[("0-", &[PHASE])]);

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs::File;

    use scallop::variables::bind;
    use tempfile::tempdir;

    use crate::command::last_command;
    use crate::macros::assert_err_re;

    use super::super::builtin_scope_tests;
    use super::run as emake;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn nonexistent() {
        assert_err_re!(emake(&[]), "^nonexistent makefile$");
    }

    #[test]
    fn command() {
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
