use scallop::ExecStatus;

use crate::shell::environment::Variable::REPLACING_VERSIONS;
use crate::shell::get_build_mut;

use super::{make_builtin, ver_test, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "ver_replacing",
    long_about = "Compare package versions being replaced with a given version."
)]
struct Command {
    op: String,
    version: String,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let build = get_build_mut();
    let versions = build
        .env
        .get(&REPLACING_VERSIONS)
        .expect("invalid build state")
        .split(' ');

    for ver in versions {
        if ver_test(&[ver, &cmd.op, &cmd.version])?.into() {
            return Ok(ExecStatus::Success);
        }
    }

    Ok(ExecStatus::Failure(1))
}

const USAGE: &str = "ver_replacing -lt 3";
make_builtin!("ver_replacing", ver_replacing_builtin);
