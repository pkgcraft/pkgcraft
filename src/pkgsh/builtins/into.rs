use scallop::builtins::ExecStatus;
use scallop::variables::bind;
use scallop::{Error, Result};

use crate::eapi::Feature;
use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "\
Takes exactly one argument and sets the value of DESTTREE.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let path = match args.len() {
        1 => match args[0] {
            "/" => "",
            s => s,
        },
        n => return Err(Error::Builtin(format!("requires 1 arg, got {n}"))),
    };

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let mut d = d.borrow_mut();
        d.desttree = path.to_string();

        if d.eapi.has(Feature::ExportDesttree) {
            bind("DESTTREE", path, None, None)?;
        }
        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "into /install/path";
make_builtin!("into", into_builtin, run, LONG_DOC, USAGE, &[("0-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use crate::pkgsh::BUILD_DATA;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as into;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(into, &[0]);
    }

    #[test]
    fn set_path() {
        into(&["/test/path"]).unwrap();
        BUILD_DATA.with(|d| {
            assert_eq!(d.borrow().desttree, "/test/path");
        });
    }
}
