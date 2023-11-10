use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Include or exclude paths for symbol stripping.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let build = get_build_mut();
    let (set, args) = match args {
        ["-x", args @ ..] => (&mut build.strip_exclude, args),
        _ => (&mut build.strip_include, args),
    };

    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".to_string()));
    }

    set.extend(args.iter().map(|s| s.to_string()));

    Ok(ExecStatus::Success)
}

const USAGE: &str = "dostrip /path/to/strip";
make_builtin!("dostrip", dostrip_builtin, run, LONG_DOC, USAGE, BUILTIN);

#[cfg(test)]
mod tests {
    use crate::macros::assert_err_re;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::BUILTIN as dostrip;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dostrip, &[0]);

        // missing args
        let r = dostrip(&["-x"]);
        assert_err_re!(r, "^requires 1 or more args, got 0");
    }

    // TODO: run builds with tests and verify file modifications

    #[test]
    fn test_include() {
        dostrip(&["/test/path"]).unwrap();
        assert!(get_build_mut().strip_include.contains("/test/path"));
    }

    #[test]
    fn test_exclude() {
        dostrip(&["-x", "/test/path"]).unwrap();
        assert!(get_build_mut().strip_exclude.contains("/test/path"));
    }
}
