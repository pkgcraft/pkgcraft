use camino::Utf8PathBuf;
use scallop::ExecStatus;

use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "docompress",
    disable_help_flag = true,
    long_about = "Include or exclude paths for compression."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(short = 'x')]
    exclude: bool,

    #[arg(required = true, allow_hyphen_values = true, value_name = "PATH")]
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

make_builtin!("docompress", docompress_builtin, true);

#[cfg(test)]
mod tests {
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, docompress};
    use super::*;

    cmd_scope_tests!("docompress /path/to/compress");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(docompress, &[0]);

        // missing args
        assert!(docompress(&["-x"]).is_err());
    }

    // TODO: run builds with tests and verify file modifications

    #[test]
    fn include() {
        for path in ["/test/path", "-"] {
            docompress(&[path]).unwrap();
            assert!(get_build_mut().compress_include.iter().any(|x| x == path));
        }
    }

    #[test]
    fn exclude() {
        for path in ["/test/path", "-"] {
            docompress(&["-x", path]).unwrap();
            assert!(get_build_mut().compress_exclude.iter().any(|x| x == path));
        }
    }
}
