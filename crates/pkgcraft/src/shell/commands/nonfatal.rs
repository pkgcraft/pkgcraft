use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "nonfatal",
    disable_help_flag = true,
    long_about = indoc::indoc! {"
        Takes one or more arguments and executes them as a command, preserving the exit
        status. If this results in a command being called that would normally abort the
        build process due to a failure, instead a non-zero exit status shall be
        returned.
    "}
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    command: String,
    #[arg(allow_hyphen_values = true)]
    args: Vec<String>,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let program = cmd.command.as_str();
    let mut command = scallop::command::Command::new(program);

    // enable nonfatal status
    let build = get_build_mut();
    build.nonfatal = true;

    // use subshell for `die` and external commands
    let subshell = program == "die" || !build.eapi().commands().contains(program);

    // run the specified command
    let result = match command.args(cmd.args).subshell(subshell).execute() {
        r @ (Ok(_) | Err(Error::Bail(_))) => r,
        Err(e) => Ok(e.into()),
    };

    // disable nonfatal status
    build.nonfatal = false;
    result
}

make_builtin!("nonfatal", nonfatal_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, functions::nonfatal};
    use super::*;

    cmd_scope_tests!("nonfatal cmd arg1 arg2");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(nonfatal, &[0]);
    }

    #[test]
    fn success() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat/pkg-1", &[]).unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        BuildData::from_raw_pkg(&raw_pkg);

        let status = nonfatal(&["ver_cut", "2-3", "1.2.3"]).unwrap();
        assert_eq!(status, ExecStatus::Success);
    }

    #[test]
    fn exit() {
        let status = nonfatal(&["exit"]).unwrap();
        assert_eq!(status, ExecStatus::Success);

        let status = nonfatal(&["exit 2"]).unwrap();
        assert_eq!(status, ExecStatus::Failure(2));
    }

    #[test]
    fn nonexistent_cmd() {
        let status = nonfatal(&["nonexistent_cmd"]).unwrap();
        assert_eq!(status, ExecStatus::Failure(1));
    }

    #[test]
    fn die() {
        let status = nonfatal(&["die", "-n", "message"]).unwrap();
        assert_eq!(status, ExecStatus::Failure(1));
    }

    #[test]
    fn invalid_builtin_scope() {
        let status = nonfatal(&["best_version", "cat/pkg"]).unwrap();
        assert_eq!(status, ExecStatus::Failure(1));
    }
}
