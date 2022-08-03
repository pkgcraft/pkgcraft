use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "Include or exclude paths for compression.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let mut d = d.borrow_mut();
        let (set, args) = match args.first() {
            Some(&"-x") => Ok((&mut d.compress_exclude, &args[1..])),
            Some(_) => Ok((&mut d.compress_include, args)),
            None => Err(Error::Base("requires 1 or more args, got 0".into())),
        }?;

        set.extend(args.iter().map(|s| s.to_string()));
        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "docompress /path/to/compress";
make_builtin!("docompress", docompress_builtin, run, LONG_DOC, USAGE, &[("4-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as docompress;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(docompress, &[0]);
    }

    #[test]
    fn test_include() {
        docompress(&["/test/path"]).unwrap();
        BUILD_DATA.with(|d| {
            assert!(d.borrow().compress_include.contains("/test/path"));
        });
    }

    #[test]
    fn test_exclude() {
        docompress(&["-x", "/test/path"]).unwrap();
        BUILD_DATA.with(|d| {
            assert!(d.borrow().compress_exclude.contains("/test/path"));
        });
    }
}
