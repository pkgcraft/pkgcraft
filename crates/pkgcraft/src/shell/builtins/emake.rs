use std::process::Command;

use scallop::builtins::ExecStatus;
use scallop::variables::{self, string_vec};
use scallop::Error;

use crate::command::RunCommand;
use crate::shell::utils::makefile_exists;
use crate::shell::write_stdout;

use super::{make_builtin, Scopes::Phases};

const LONG_DOC: &str = "Run the make command for a package.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if !makefile_exists() {
        return Err(Error::Base("nonexistent makefile".to_string()));
    }

    // determine make program to run
    let make_prog = variables::optional("MAKE");
    let make_prog = make_prog.as_deref().unwrap_or("make");
    let mut emake = Command::new(make_prog);

    // inject user options
    if let Some(opts) = string_vec("MAKEOPTS") {
        emake.args(&opts);
    }

    // arguments override user options
    emake.args(args);

    write_stdout!("{}", emake.to_vec().join(" "))?;
    emake.run()?;
    Ok(ExecStatus::Success)
}

const USAGE: &str = "emake -C builddir";
make_builtin!("emake", emake_builtin, run, LONG_DOC, USAGE, [("..", [Phases])]);

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs::File;

    use scallop::variables::bind;
    use tempfile::tempdir;

    use crate::command::commands;
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
        let cmd = commands().pop().unwrap();
        assert_eq!(cmd[0], "make");

        // custom args
        let args = ["-C", "build", "install"];
        emake(&args).unwrap();
        let cmd = commands().pop().unwrap();
        assert_eq!(cmd[1..], args);

        // using $MAKEOPTS settings
        bind("MAKEOPTS", "-j10", None, None).unwrap();
        emake(&[]).unwrap();
        let cmd = commands().pop().unwrap();
        assert_eq!(cmd[1..], ["-j10"]);
        bind("MAKEOPTS", "-j20 -l 20", None, None).unwrap();
        emake(&[]).unwrap();
        let cmd = commands().pop().unwrap();
        assert_eq!(cmd[1..], ["-j20", "-l", "20"]);
        // args override $MAKEOPTS
        emake(&["-j1"]).unwrap();
        let cmd = commands().pop().unwrap();
        assert_eq!(cmd[1..], ["-j20", "-l", "20", "-j1"]);

        // custom $MAKE prog
        bind("MAKE", "custom-make", None, None).unwrap();
        emake(&[]).unwrap();
        let cmd = commands().pop().unwrap();
        assert_eq!(cmd[0], "custom-make");
    }
}
