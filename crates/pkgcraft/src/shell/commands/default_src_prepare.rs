use scallop::ExecStatus;

use super::_default_phase_func::default_phase_func;
use super::make_builtin;

const LONG_DOC: &str = "\
Runs the default src_prepare implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    default_phase_func(args)
}

const USAGE: &str = "default_src_prepare";
make_builtin!("default_src_prepare", default_src_prepare_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::pkg::Build;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::{get_build_mut, BuildData};
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_args, cmd_scope_tests, default_src_prepare};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_prepare, &[1]);
    }

    #[test]
    fn valid_phase() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing default_src_prepare command"
            SLOT=0
            VAR=1
            src_prepare() {
                default_src_prepare
                VAR=2
            }
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
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
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing default_src_prepare command"
            SLOT=0
            VAR=1
            pkg_setup() {
                default_src_prepare
                VAR=2
            }
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        let r = pkg.build();
        assert_err_re!(r, "line 6: default_src_prepare: error: disabled in pkg_setup scope$");
        // verify custom pkg_setup() stopped on error
        assert_eq!(scallop::variables::optional("VAR").as_deref(), Some("1"));
    }
}
