use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkgsh::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "\
Takes exactly one argument and sets the install path for dodoc and other doc-related commands.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let path = match args.len() {
        1 => match args[0] {
            "/" => Ok(""),
            s => Ok(s),
        },
        n => Err(Error::Base(format!("requires 1 arg, got {n}"))),
    }?;

    get_build_mut().docdesttree = path.to_string();

    Ok(ExecStatus::Success)
}

const USAGE: &str = "docinto /install/path";
make_builtin!("docinto", docinto_builtin, run, LONG_DOC, USAGE, &[("..", &["src_install"])]);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as docinto;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(docinto, &[0, 2]);
    }

    #[test]
    fn set_path() {
        docinto(&["examples"]).unwrap();
        assert_eq!(get_build_mut().docdesttree, "examples");
    }
}
