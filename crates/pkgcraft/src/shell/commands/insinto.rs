use scallop::ExecStatus;

use crate::shell::environment::Variable::INSDESTTREE;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "insinto",
    disable_help_flag = true,
    long_about = "Takes exactly one argument and sets the value of INSDESTTREE."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(allow_hyphen_values = true)]
    path: String,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    INSDESTTREE.set(cmd.path)?;
    Ok(ExecStatus::Success)
}

make_builtin!("insinto", insinto_builtin);

#[cfg(test)]
mod tests {
    use scallop::variables;

    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::phase::PhaseKind;
    use crate::shell::{BuildData, Scope, get_build_mut};

    use super::super::{assert_invalid_cmd, cmd_scope_tests, functions::insinto};
    use super::*;

    cmd_scope_tests!("insinto /install/path");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(insinto, &[0]);
    }

    #[test]
    fn set_path() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            temp.create_ebuild("cat/pkg-1", &[&format!("EAPI={eapi}")])
                .unwrap();
            let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
            BuildData::from_raw_pkg(&raw_pkg);
            let build = get_build_mut();
            build.scope = Scope::Phase(PhaseKind::SrcInstall);
            insinto(&["/test/path"]).unwrap();
            assert_eq!(build.env(&INSDESTTREE), "/test/path");

            // verify conditional EAPI environment export
            let build_var = eapi.env().get(&INSDESTTREE).unwrap();
            let env_val = variables::optional("INSDESTTREE");
            if build_var.is_allowed(&build.scope) {
                assert_eq!(env_val.unwrap(), "/test/path");
            } else {
                assert!(env_val.is_none());
            }

            insinto(&["-"]).unwrap();
            assert_eq!(build.env(&INSDESTTREE), "-");
        }
    }
}
