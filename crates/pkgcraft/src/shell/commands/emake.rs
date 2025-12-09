use std::io::Write;

use scallop::variables::{self, string_vec};
use scallop::{Error, ExecStatus};

use crate::command::RunCommand;
use crate::io::stdout;
use crate::shell::utils::makefile_exists;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "emake",
    disable_help_flag = true,
    long_about = "Run the make command for a package."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(allow_hyphen_values = true)]
    args: Vec<String>,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;

    if !makefile_exists() {
        return Err(Error::Base("nonexistent makefile".to_string()));
    }

    // determine make program to run
    let make_prog = variables::optional("MAKE");
    let make_prog = make_prog.as_deref().unwrap_or("make");
    let mut emake = std::process::Command::new(make_prog);

    // inject user options
    if let Some(opts) = string_vec("MAKEOPTS") {
        emake.args(&opts);
    }

    // arguments override user options
    emake.args(&cmd.args);

    writeln!(stdout(), "{}", emake.to_vec().join(" "))?;
    emake.run()?;
    Ok(ExecStatus::Success)
}

make_builtin!("emake", emake_builtin);

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs::File;

    use scallop::variables::{bind, unbind};
    use tempfile::tempdir;

    use crate::command::commands;
    use crate::test::assert_err_re;

    use super::super::{cmd_scope_tests, functions::emake};
    use super::*;

    cmd_scope_tests!("emake -C builddir");

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

        // force make defaults
        unbind("MAKE").unwrap();
        unbind("MAKEOPTS").unwrap();

        // default make prog
        emake(&[]).unwrap();
        let cmd = commands().pop().unwrap();
        assert_eq!(cmd[0], "make");
        assert_eq!(stdout().get(), "make\n");

        // custom args
        let args = ["-C", "build", "install"];
        emake(&args).unwrap();
        let cmd = commands().pop().unwrap();
        assert_eq!(cmd[1..], args);
        assert_eq!(stdout().get(), "make -C build install\n");

        // using $MAKEOPTS settings
        bind("MAKEOPTS", "-j10", None, None).unwrap();
        emake(&[]).unwrap();
        let cmd = commands().pop().unwrap();
        assert_eq!(cmd[1..], ["-j10"]);
        assert_eq!(stdout().get(), "make -j10\n");
        bind("MAKEOPTS", "-j20 -l 20", None, None).unwrap();
        emake(&[]).unwrap();
        let cmd = commands().pop().unwrap();
        assert_eq!(cmd[1..], ["-j20", "-l", "20"]);
        assert_eq!(stdout().get(), "make -j20 -l 20\n");
        // args override $MAKEOPTS
        emake(&["-j1"]).unwrap();
        let cmd = commands().pop().unwrap();
        assert_eq!(cmd[1..], ["-j20", "-l", "20", "-j1"]);
        assert_eq!(stdout().get(), "make -j20 -l 20 -j1\n");

        // custom $MAKE prog
        bind("MAKE", "custom-make", None, None).unwrap();
        emake(&[]).unwrap();
        let cmd = commands().pop().unwrap();
        assert_eq!(cmd[0], "custom-make");
        assert_eq!(stdout().get(), "custom-make -j20 -l 20\n");
    }
}
