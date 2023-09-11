use scallop::builtins::ExecStatus;

use crate::shell::phase::PhaseKind::PkgNofetch;

use super::_default_phase_func::default_phase_func;
use super::{make_builtin, Scopes::Phase};

const LONG_DOC: &str = "\
Runs the default pkg_nofetch implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    default_phase_func(args)
}

const USAGE: &str = "default_pkg_nofetch";
make_builtin!(
    "default_pkg_nofetch",
    default_pkg_nofetch_builtin,
    run,
    LONG_DOC,
    USAGE,
    &[("2..", &[Phase(PkgNofetch)])]
);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::pkg::BuildablePackage;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as default_pkg_nofetch;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_pkg_nofetch, &[1]);
    }

    #[test]
    fn invalid_phase() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing default_pkg_nofetch command"
            SLOT=0
            VAR=1
            pkg_setup() {
                default_pkg_nofetch
                VAR=2
            }
        "#};
        let pkg = t.create_pkg_from_str("cat/pkg-1", data).unwrap();
        BuildData::from_pkg(&pkg);
        let result = pkg.build();
        assert_err_re!(result, "pkg_setup scope doesn't enable command: default_pkg_nofetch$");
        // verify custom pkg_setup() stopped on error
        assert_eq!(scallop::variables::optional("VAR").as_deref(), Some("1"));
    }
}
