use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Include or exclude paths for compression.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let build = get_build_mut();
    let (set, args) = match args {
        ["-x", args @ ..] => (&mut build.compress_exclude, args),
        _ => (&mut build.compress_include, args),
    };

    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".to_string()));
    }

    set.extend(args.iter().map(|s| s.to_string()));

    Ok(ExecStatus::Success)
}

const USAGE: &str = "docompress /path/to/compress";
make_builtin!("docompress", docompress_builtin);

#[cfg(test)]
mod tests {
    use crate::macros::assert_err_re;

    use super::super::{assert_invalid_args, cmd_scope_tests, docompress};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(docompress, &[0]);

        // missing args
        let r = docompress(&["-x"]);
        assert_err_re!(r, "^requires 1 or more args, got 0");
    }

    // TODO: run builds with tests and verify file modifications

    #[test]
    fn test_include() {
        docompress(&["/test/path"]).unwrap();
        assert!(get_build_mut().compress_include.contains("/test/path"));
    }

    #[test]
    fn test_exclude() {
        docompress(&["-x", "/test/path"]).unwrap();
        assert!(get_build_mut().compress_exclude.contains("/test/path"));
    }
}
