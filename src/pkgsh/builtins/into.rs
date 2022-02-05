use scallop::builtins::{output_error_func, Builtin, ExecStatus};
use scallop::variables::bind;
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "\
Takes exactly one argument and sets the value of DESTTREE.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let path = match args.len() {
        1 => match args[0] {
            "/" => "",
            s => s,
        },
        n => return Err(Error::new(format!("requires 1 arg, got {}", n))),
    };

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let mut d = d.borrow_mut();
        d.desttree = path.to_string();

        if d.eapi.has("export_desttree") {
            bind("DESTTREE", path, None, None)?;
        }
        Ok(ExecStatus::Success)
    })
}

pub static BUILTIN: Builtin = Builtin {
    name: "into",
    func: run,
    help: LONG_DOC,
    usage: "into /install/path",
    error_func: Some(output_error_func),
};

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as into;
    use crate::pkgsh::BUILD_DATA;

    use rusty_fork::rusty_fork_test;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(into, vec![0]);
        }

        #[test]
        fn set_path() {
            into(&["/test/path"]).unwrap();
            BUILD_DATA.with(|d| {
                assert_eq!(d.borrow().desttree, "/test/path");
            });
        }
    }
}
