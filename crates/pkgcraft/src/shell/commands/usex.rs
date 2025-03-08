use std::io::Write;

use scallop::ExecStatus;

use crate::io::stdout;

use super::{make_builtin, use_, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "usex",
    long_about = "Tests if a given USE flag is enabled and outputs a string related to its status."
)]
struct Command {
    flag: String,
    #[arg(required = false, default_value = "yes")]
    enabled: String,
    #[arg(required = false, default_value = "no")]
    disabled: String,
    #[arg(required = false, default_value = "")]
    enabled_output: String,
    #[arg(required = false, default_value = "")]
    disabled_output: String,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let mut stdout = stdout();
    match use_(&[&cmd.flag])? {
        ExecStatus::Success => write!(stdout, "{}{}", cmd.enabled, cmd.enabled_output)?,
        ExecStatus::Failure(_) => write!(stdout, "{}{}", cmd.disabled, cmd.disabled_output)?,
    }
    stdout.flush()?;

    Ok(ExecStatus::Success)
}

make_builtin!("usex", usex_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::pkg::Build;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::{get_build_mut, BuildData};
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, usex};
    use super::*;

    cmd_scope_tests!("usex flag");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(usex, &[0, 6]);
    }

    #[test]
    fn empty_iuse_effective() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        assert_err_re!(usex(&["use"]), "^USE flag not in IUSE: use$");
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
        for (args, expected) in [
            (vec!["use"], "no"),
            (vec!["use", "arg2", "arg3", "arg4", "arg5"], "arg3arg5"),
            (vec!["!use"], "yes"),
            (vec!["!use", "arg2", "arg3", "arg4", "arg5"], "arg2arg4"),
        ] {
            usex(&args).unwrap();
            assert_eq!(stdout().get(), expected);
        }

        // enabled
        get_build_mut().use_.insert("use".to_string());
        for (args, expected) in [
            (vec!["use"], "yes"),
            (vec!["use", "arg2", "arg3", "arg4", "arg5"], "arg2arg4"),
            (vec!["!use"], "no"),
            (vec!["!use", "arg2", "arg3", "arg4", "arg5"], "arg3arg5"),
        ] {
            usex(&args).unwrap();
            assert_eq!(stdout().get(), expected);
        }
    }

    #[ignore]
    #[test]
    fn subshell() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="subshell usex success"
                SLOT=0
                IUSE="use1 use2"
                pkg_setup() {{
                    local disabled=$(usex use1)
                    [[ ${{disabled}} == "no" ]] || die "usex failed disabled"
                    local enabled=$(usex use2)
                    [[ ${{enabled}} == "yes" ]] || die "usex failed enabled"
                }}
            "#};
            temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
            let pkg = repo.get_pkg("cat/pkg-1").unwrap();
            BuildData::from_pkg(&pkg);
            get_build_mut().use_.insert("use2".to_string());
            let r = pkg.build();
            assert!(r.is_ok(), "{}", r.unwrap_err());

            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="subshell usex failure"
                SLOT=0
                IUSE="use1 use2"
                VAR=1
                pkg_setup() {{
                    local disabled=$(usex)
                    VAR=2
                }}
            "#};
            temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
            let pkg = repo.get_pkg("cat/pkg-1").unwrap();
            BuildData::from_pkg(&pkg);
            let r = pkg.build();
            assert_err_re!(r, "line 7: usex: error: requires 1 to 5 args, got 0$");
            assert_eq!(scallop::variables::optional("VAR").as_deref(), Some("1"));
        }
    }
}
