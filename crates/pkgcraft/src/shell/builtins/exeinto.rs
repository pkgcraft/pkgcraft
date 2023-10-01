use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::SrcInstall;

use super::make_builtin;

const LONG_DOC: &str = "\
Takes exactly one argument and sets the install path for doexe and newexe.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let path = match args[..] {
        ["/"] => "",
        [s] => s,
        _ => return Err(Error::Base(format!("requires 1 arg, got {}", args.len()))),
    };

    get_build_mut().exedesttree = path.to_string();

    Ok(ExecStatus::Success)
}

const USAGE: &str = "exeinto /install/path";
make_builtin!("exeinto", exeinto_builtin, run, LONG_DOC, USAGE, [("..", [SrcInstall])]);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as exeinto;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(exeinto, &[0, 2]);
    }

    #[test]
    fn set_path() {
        exeinto(&["/test/path"]).unwrap();
        assert_eq!(get_build_mut().exedesttree, "/test/path");
    }
}
