use scallop::ExecStatus;

use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "default_src_unpack",
    long_about = indoc::indoc! {"
        Runs the default src_unpack implementation for a package's EAPI.
    "}
)]
struct Command;

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let _cmd = Command::try_parse_args(args)?;
    get_build_mut().phase().default()
}

make_builtin!("default_src_unpack", default_src_unpack_builtin);

#[cfg(test)]
mod tests {
    use super::super::{assert_invalid_cmd, cmd_scope_tests, default_src_unpack};
    use super::*;

    cmd_scope_tests!("default_src_unpack");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(default_src_unpack, &[1]);
    }

    // TODO: add usage tests
}
