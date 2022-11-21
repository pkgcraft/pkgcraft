use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkgsh::{BuildVariable, BUILD_DATA};

use super::make_builtin;

const LONG_DOC: &str = "\
Takes exactly one argument and sets the value of INSDESTTREE.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let path = match args.len() {
        1 => match args[0] {
            "/" => Ok(""),
            s => Ok(s),
        },
        n => Err(Error::Base(format!("requires 1 arg, got {n}"))),
    }?;

    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let mut d = d.borrow_mut();
        d.insdesttree = path.to_string();
        d.override_var(BuildVariable::INSDESTTREE, path)?;
        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "insinto /install/path";
make_builtin!("insinto", insinto_builtin, run, LONG_DOC, USAGE, &[("0-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use scallop::{shell, variables};

    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::pkgsh::phase::{Phase, PHASE_STUB};
    use crate::pkgsh::{BuildData, BuildVariable, Scope, BUILD_DATA};

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
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, _) = config.temp_repo("test", 0).unwrap();
        let (_, cpv) = t.create_ebuild("cat/pkg-1", []).unwrap();

        for eapi in EAPIS_OFFICIAL.values() {
            BuildData::update(&cpv, "test");
            BUILD_DATA.with(|d| {
                let phase = Phase::SrcInstall(PHASE_STUB);
                d.borrow_mut().phase = Some(phase);
                d.borrow_mut().scope = Scope::Phase(phase);
                d.borrow_mut().eapi = eapi;
                insinto(&["/test/path"]).unwrap();
                assert_eq!(d.borrow().insdesttree, "/test/path");

                // verify conditional EAPI environment export
                let env_val = variables::optional("INSDESTTREE");
                match eapi.env().contains_key(&BuildVariable::INSDESTTREE) {
                    true => assert_eq!(env_val.unwrap(), "/test/path"),
                    false => assert!(env_val.is_none()),
                }
            });

            // reset shell env
            shell::Shell::reset();
        }
    }
}
