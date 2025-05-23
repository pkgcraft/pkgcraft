use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::{TryParseArgs, UseFlag, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "use",
    disable_help_flag = true,
    long_about = indoc::indoc! {"
        Returns success if the USE flag argument is enabled, failure otherwise.
        The return values are inverted if the flag name is prefixed with !.
    "}
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    use_flag: UseFlag,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let flag = &cmd.use_flag.flag;
    let build = get_build_mut();
    let pkg = build.ebuild_pkg();

    if !pkg.iuse_effective().contains(flag) {
        return Err(Error::Base(format!("USE flag not in IUSE: {flag}")));
    }

    let ret = build.use_.contains(flag) ^ cmd.use_flag.inverted;
    Ok(ExecStatus::from(ret))
}

make_builtin!("use", use_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, use_};
    use super::*;

    cmd_scope_tests!("use flag");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(use_, &[0, 2]);
    }

    #[test]
    fn empty_iuse_effective() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        assert_err_re!(use_(&["use"]), "^USE flag not in IUSE: use$");
    }

    #[test]
    fn enabled_and_disabled() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat/pkg-1", &["IUSE=use"]).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        // disabled
        assert_eq!(use_(&["use"]).unwrap(), ExecStatus::Failure(1));
        // inverted check
        assert_eq!(use_(&["!use"]).unwrap(), ExecStatus::Success);

        // enabled
        get_build_mut().use_.insert("use".to_string());
        // use flag is enabled
        assert_eq!(use_(&["use"]).unwrap(), ExecStatus::Success);
        // inverted check
        assert_eq!(use_(&["!use"]).unwrap(), ExecStatus::Failure(1));
    }
}
