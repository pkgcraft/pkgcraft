use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "\
Takes exactly one argument and sets the install path for dodoc and other doc-related commands.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let path = match args.len() {
        1 => match args[0] {
            "/" => "",
            s => s,
        },
        n => return Err(Error::Builtin(format!("requires 1 arg, got {}", n))),
    };

    BUILD_DATA.with(|d| {
        d.borrow_mut().docdesttree = path.to_string();
    });

    Ok(ExecStatus::Success)
}

pub static BUILTIN: Builtin = Builtin {
    name: "docinto",
    func: run,
    help: LONG_DOC,
    usage: "docinto /install/path",
};

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as docinto;
    use crate::pkgsh::BUILD_DATA;

    use rusty_fork::rusty_fork_test;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(docinto, &[0, 2]);
        }

        #[test]
        fn set_path() {
            docinto(&["examples"]).unwrap();
            BUILD_DATA.with(|d| {
                assert_eq!(d.borrow().docdesttree, "examples");
            });
        }
    }
}
