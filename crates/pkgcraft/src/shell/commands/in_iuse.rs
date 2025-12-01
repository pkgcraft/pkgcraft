use scallop::ExecStatus;

use crate::shell::get_build_mut;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "in_iuse",
    disable_help_flag = true,
    long_about = "Returns success if the USE flag argument is found in IUSE_EFFECTIVE."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(allow_hyphen_values = true)]
    flag: String,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let pkg = get_build_mut().ebuild_pkg();
    Ok(ExecStatus::from(pkg.iuse_effective().contains(&cmd.flag)))
}

make_builtin!("in_iuse", in_iuse_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, functions::in_iuse};
    use super::*;

    cmd_scope_tests!("in_iuse flag");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(in_iuse, &[0, 2]);
    }

    #[test]
    fn known_and_unknown() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat/pkg-1", &["IUSE=use"]).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        // unknown
        assert_eq!(in_iuse(&["unknown"]).unwrap(), ExecStatus::Failure(1));

        // known
        assert_eq!(in_iuse(&["use"]).unwrap(), ExecStatus::Success);
    }
}
