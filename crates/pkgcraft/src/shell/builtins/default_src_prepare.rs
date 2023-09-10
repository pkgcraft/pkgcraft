use scallop::builtins::ExecStatus;

use crate::shell::phase::PhaseKind::SrcPrepare;

use super::_default_phase_func::default_phase_func;
use super::{make_builtin, Scopes::Phase};

const LONG_DOC: &str = "\
Runs the default src_prepare implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    default_phase_func(args)
}

const USAGE: &str = "default_src_prepare";
make_builtin!(
    "default_src_prepare",
    default_src_prepare_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("2..", &[Phase(SrcPrepare)])]
);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::pkg::BuildablePackage;
    use crate::shell::{get_build_mut, BuildData};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as default_src_prepare;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_prepare, &[1]);
    }

    #[test]
    fn valid_phase() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let data = indoc::formatdoc! {r#"
            EAPI=8
            DESCRIPTION="testing default_src_prepare command"
            SLOT=0
            VAR=1
            src_prepare() {{
                default_src_prepare
                VAR=2
            }}
        "#};
        let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
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
        let t = config.temp_repo("test", 0, None).unwrap();
        let data = indoc::formatdoc! {r#"
            EAPI=8
            DESCRIPTION="testing default_src_prepare command"
            SLOT=0
            VAR=1
            pkg_setup() {{
                default_src_prepare
                VAR=2
            }}
        "#};
        let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
        BuildData::from_pkg(&pkg);
        let result = pkg.build();
        assert_err_re!(result, "pkg_setup scope doesn't enable command: default_src_prepare$");
        // verify custom pkg_setup() stopped on error
        assert_eq!(scallop::variables::optional("VAR").as_deref(), Some("1"));
    }
}
