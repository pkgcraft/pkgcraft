use std::io::Write;

use scallop::ExecStatus;
use scallop::command::current_command_string;

use crate::io::stderr;
use crate::shell::get_build_mut;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "edo",
    disable_help_flag = true,
    long_about = "Takes a command line, prints it to stderr and executes the command."
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

    // use subshell for `die` and external commands
    let build = get_build_mut();
    let subshell = program == "die" || !build.eapi().commands().contains(program);

    // output command to stderr
    let command_string = current_command_string()?;
    let msg = command_string
        .strip_prefix("edo ")
        .expect("invalid `edo` call");
    writeln!(stderr(), "{msg}")?;

    // run specified command
    command.args(cmd.args).subshell(subshell).execute()
}

make_builtin!("edo", edo_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::pkg::Build;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, functions::edo};
    use super::*;

    cmd_scope_tests!("edo cmd arg1 arg2");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(edo, &[0]);
    }

    #[test]
    fn success() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();

        let data = indoc::indoc! {r#"
            EAPI=9
            DESCRIPTION="testing edo"
            SLOT=0
            S=${WORKDIR}

            src_configure() {
                edo echo 1 2 3 $foo ${bar} "" "white space"
            }
        "#};

        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();

        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        let r = pkg.build();
        assert!(r.is_ok(), "{}", r.unwrap_err());
        assert_eq!(stderr().get(), "echo 1 2 3 $foo ${bar} \"\" \"white space\"\n");
    }

    #[test]
    fn failure() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();

        let data = indoc::indoc! {r#"
            EAPI=9
            DESCRIPTION="testing edo"
            SLOT=0
            S=${WORKDIR}

            src_configure() {
                edo nonexistent arg1 arg2
            }
        "#};

        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();

        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        let r = pkg.build();
        assert_err_re!(
            r,
            "^cat/pkg-1::test: line 7: edo: error: unknown command: nonexistent$"
        );
    }

    #[test]
    fn nonfatal() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();

        let data = indoc::indoc! {r#"
            EAPI=9
            DESCRIPTION="testing edo"
            SLOT=0
            S=${WORKDIR}

            src_configure() {
                nonfatal edo nonexistent arg1 arg2
                [[ $? == 1 ]] || die "command didn't fail"
            }
        "#};

        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();

        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        let r = pkg.build();
        assert!(r.is_ok(), "{}", r.unwrap_err());
        assert_eq!(stderr().get(), "nonexistent arg1 arg2\n");
    }
}
