use scallop::{Error, ExecStatus};

use crate::shell::environment::Variable::INSDESTTREE;
use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::SrcInstall;

use super::make_builtin;

const LONG_DOC: &str = "\
Takes exactly one argument and sets the value of INSDESTTREE.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let path = match args[..] {
        ["/"] => "",
        [s] => s,
        _ => return Err(Error::Base(format!("requires 1 arg, got {}", args.len()))),
    };

    let build = get_build_mut();
    build.insdesttree = path.to_string();
    build.override_var(INSDESTTREE, path)?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "insinto /install/path";
make_builtin!("insinto", insinto_builtin, run, LONG_DOC, USAGE, [("..", [SrcInstall])]);

#[cfg(test)]
mod tests {
    use scallop::variables;

    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::shell::phase::PhaseKind;
    use crate::shell::{BuildData, Scope};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as insinto;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(insinto, &[0]);
    }

    #[test]
    fn set_path() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        for eapi in EAPIS_OFFICIAL.iter() {
            let raw_pkg = t
                .create_raw_pkg("cat/pkg-1", &[&format!("EAPI={eapi}")])
                .unwrap();
            BuildData::from_raw_pkg(&raw_pkg);
            let build = get_build_mut();
            build.scope = Scope::Phase(PhaseKind::SrcInstall);
            insinto(&["/test/path"]).unwrap();
            assert_eq!(build.insdesttree, "/test/path");

            // verify conditional EAPI environment export
            let env_val = variables::optional("INSDESTTREE");
            if eapi.env().contains(&INSDESTTREE) {
                assert_eq!(env_val.unwrap(), "/test/path");
            } else {
                assert!(env_val.is_none());
            }
        }
    }
}
