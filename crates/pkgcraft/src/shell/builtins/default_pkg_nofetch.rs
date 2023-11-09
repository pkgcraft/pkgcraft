use scallop::ExecStatus;

use super::_default_phase_func::default_phase_func;
use super::make_builtin;

const LONG_DOC: &str = "\
Runs the default pkg_nofetch implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    default_phase_func(args)
}

const USAGE: &str = "default_pkg_nofetch";
make_builtin!("default_pkg_nofetch", default_pkg_nofetch_builtin, run, LONG_DOC, USAGE, BUILTIN);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::pkg::BuildPackage;
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
        let r = pkg.build();
        assert_err_re!(r, "line 6: default_pkg_nofetch: error: disabled in pkg_setup scope$");
        // verify custom pkg_setup() stopped on error
        assert_eq!(scallop::variables::optional("VAR").as_deref(), Some("1"));
    }
}
