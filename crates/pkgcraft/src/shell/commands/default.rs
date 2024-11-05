use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Calls the default_* function for the current phase.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if !args.is_empty() {
        return Err(Error::Base(format!("takes no args, got {}", args.len())));
    }

    let build = get_build_mut();
    let phase = build.phase()?;
    let default_phase = format!("default_{phase}");

    if let Some(cmd) = build.eapi().commands().get(default_phase.as_str()) {
        cmd(&[])
    } else {
        Err(Error::Base(format!("{phase} phase has no default")))
    }
}

const USAGE: &str = "default";
make_builtin!("default", default_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::pkg::Build;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_args, cmd_scope_tests, default};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(default, &[1]);
    }

    #[test]
    fn valid_phase() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing default command"
            SLOT=0
            VAR=1
            src_prepare() {
                default
                VAR=2
            }
        "#};
        let pkg = temp.create_pkg_from_str("cat/pkg-1", data).unwrap();
        BuildData::from_pkg(&pkg);
        pkg.build().unwrap();
        // verify default src_prepare() was run
        assert!(get_build_mut().user_patches_applied);
        // verify custom src_prepare() was run
        assert_eq!(scallop::variables::optional("VAR").as_deref(), Some("2"));
    }

    #[test]
    fn invalid_phase() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing default command"
            SLOT=0
            VAR=1
            pkg_setup() {
                default
                VAR=2
            }
        "#};
        let pkg = temp.create_pkg_from_str("cat/pkg-1", data).unwrap();
        BuildData::from_pkg(&pkg);
        let result = pkg.build();
        assert_err_re!(result, "line 6: default: error: pkg_setup phase has no default$");
        // verify custom pkg_setup() stopped on error
        assert_eq!(scallop::variables::optional("VAR").as_deref(), Some("1"));
    }
}
