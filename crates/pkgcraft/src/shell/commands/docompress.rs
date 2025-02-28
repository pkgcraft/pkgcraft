use camino::Utf8PathBuf;
use scallop::ExecStatus;

use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "docompress",
    long_about = "Include or exclude paths for compression."
)]
struct Command {
    #[arg(short = 'x')]
    exclude: bool,
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let build = get_build_mut();

    if cmd.exclude {
        build.compress_exclude.extend(cmd.paths);
    } else {
        build.compress_include.extend(cmd.paths);
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "docompress /path/to/compress";
make_builtin!("docompress", docompress_builtin);

#[cfg(test)]
mod tests {
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, docompress};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(docompress, &[0]);

        // missing args
        assert!(docompress(&["-x"]).is_err());
    }

    // TODO: run builds with tests and verify file modifications

    #[test]
    fn include() {
        docompress(&["/test/path"]).unwrap();
        assert!(get_build_mut()
            .compress_include
            .iter()
            .any(|x| x == "/test/path"));
    }

    #[test]
    fn exclude() {
        docompress(&["-x", "/test/path"]).unwrap();
        assert!(get_build_mut()
            .compress_exclude
            .iter()
            .any(|x| x == "/test/path"));
    }
}
