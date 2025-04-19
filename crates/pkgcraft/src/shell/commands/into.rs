use scallop::ExecStatus;

use crate::shell::environment::Variable::DESTTREE;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "into",
    disable_help_flag = true,
    long_about = "Takes exactly one argument and sets the value of DESTTREE."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(allow_hyphen_values = true)]
    path: String,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    DESTTREE.set(cmd.path)?;
    Ok(ExecStatus::Success)
}

make_builtin!("into", into_builtin);

#[cfg(test)]
mod tests {
    use scallop::variables;

    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::phase::PhaseKind;
    use crate::shell::{get_build_mut, BuildData, Scope};

    use super::super::{assert_invalid_cmd, cmd_scope_tests, into};
    use super::*;

    cmd_scope_tests!("into /install/path");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(into, &[0]);
    }

    #[test]
    fn set_path() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            temp.create_ebuild("cat/pkg-1", &[&format!("EAPI={eapi}")])
                .unwrap();
            let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
            BuildData::from_raw_pkg(&raw_pkg);
            let build = get_build_mut();
            build.scope = Scope::Phase(PhaseKind::SrcInstall);
            into(&["/test/path"]).unwrap();
            assert_eq!(build.env(DESTTREE), "/test/path");

            // verify conditional EAPI environment export
            let build_var = eapi.env().get(&DESTTREE).unwrap();
            let env_val = variables::optional("DESTTREE");
            if build_var.is_allowed(&build.scope) {
                assert_eq!(env_val.unwrap(), "/test/path")
            } else {
                assert!(env_val.is_none())
            }

            into(&["-"]).unwrap();
            assert_eq!(build.env(DESTTREE), "-");
        }
    }
}
