use scallop::ExecStatus;

use crate::shell::environment::Variable::EXEDESTTREE;
use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "exeinto",
    long_about = "Takes exactly one argument and sets the install path for doexe and newexe."
)]
struct Command {
    path: String,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    get_build_mut().override_var(EXEDESTTREE, &cmd.path)?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "exeinto /install/path";
make_builtin!("exeinto", exeinto_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_cmd, cmd_scope_tests, exeinto};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(exeinto, &[0, 2]);
    }

    #[test]
    fn set_path() {
        exeinto(&["/test/path"]).unwrap();
        assert_eq!(get_build_mut().env(EXEDESTTREE), "/test/path");
    }
}
