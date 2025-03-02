use std::io::Write;

use scallop::{Error, ExecStatus};

use crate::eapi::Feature::UsevTwoArgs;
use crate::io::stdout;
use crate::shell::get_build_mut;

use super::{make_builtin, use_, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "usev",
    long_about = "The same as use, but also prints the flag name if the condition is met."
)]
struct Command {
    flag: String,
    output: Option<String>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let eapi = get_build_mut().eapi();
    let flag = &cmd.flag;
    let output = if let Some(value) = cmd.output.as_deref() {
        if !eapi.has(UsevTwoArgs) {
            return Err(Error::Base(format!("EAPI {eapi}: output argument unsupported")));
        }
        value
    } else {
        flag.strip_prefix('!').unwrap_or(flag)
    };

    let ret = use_(&[flag])?;
    if bool::from(&ret) {
        write!(stdout(), "{output}")?;
    }

    Ok(ret)
}

const USAGE: &str = "usev flag";
make_builtin!("usev", usev_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, usev};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(usev, &[0, 3]);

        for eapi in EAPIS_OFFICIAL.iter().filter(|e| !e.has(UsevTwoArgs)) {
            BuildData::empty(eapi);
            assert_invalid_cmd(usev, &[2]);
        }
    }

    #[test]
    fn empty_iuse_effective() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        assert_err_re!(usev(&["use"]), "^USE flag not in IUSE: use$");
    }

    #[test]
    fn enabled_and_disabled() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat/pkg-1", &["IUSE=use"]).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        // disabled
        for (args, status, expected) in
            [(&["use"], ExecStatus::Failure(1), ""), (&["!use"], ExecStatus::Success, "use")]
        {
            assert_eq!(usev(args).unwrap(), status);
            assert_eq!(stdout().get(), expected);
        }

        // check EAPIs that support two arg variant
        for eapi in EAPIS_OFFICIAL.iter().filter(|e| e.has(UsevTwoArgs)) {
            temp.create_ebuild("cat/pkg-1", &["IUSE=use", &format!("EAPI={eapi}")])
                .unwrap();
            let pkg = repo.get_pkg("cat/pkg-1").unwrap();
            BuildData::from_pkg(&pkg);

            for (args, status, expected) in [
                (&["use", "out"], ExecStatus::Failure(1), ""),
                (&["!use", "out"], ExecStatus::Success, "out"),
            ] {
                assert_eq!(usev(args).unwrap(), status);
                assert_eq!(stdout().get(), expected);
            }
        }

        // enabled
        temp.create_ebuild("cat/pkg-1", &["IUSE=use"]).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        get_build_mut().use_.insert("use".to_string());

        for (args, status, expected) in
            [(&["use"], ExecStatus::Success, "use"), (&["!use"], ExecStatus::Failure(1), "")]
        {
            assert_eq!(usev(args).unwrap(), status);
            assert_eq!(stdout().get(), expected);
        }

        // check EAPIs that support two arg variant
        for eapi in EAPIS_OFFICIAL.iter().filter(|e| e.has(UsevTwoArgs)) {
            temp.create_ebuild("cat/pkg-1", &["IUSE=use", &format!("EAPI={eapi}")])
                .unwrap();
            let pkg = repo.get_pkg("cat/pkg-1").unwrap();
            BuildData::from_pkg(&pkg);
            get_build_mut().use_.insert("use".to_string());

            for (args, status, expected) in [
                (&["use", "out"], ExecStatus::Success, "out"),
                (&["!use", "out"], ExecStatus::Failure(1), ""),
            ] {
                assert_eq!(usev(args).unwrap(), status);
                assert_eq!(stdout().get(), expected);
            }
        }
    }
}
