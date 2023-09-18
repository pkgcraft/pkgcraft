use scallop::builtins::ExecStatus;

use crate::shell::phase::PhaseKind::SrcInstall;

use super::_default_phase_func::default_phase_func;
use super::make_builtin;

const LONG_DOC: &str = "\
Runs the default src_install implementation for a package's EAPI.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    default_phase_func(args)
}

const USAGE: &str = "default_src_install";
make_builtin!(
    "default_src_install",
    default_src_install_builtin,
    run,
    LONG_DOC,
    USAGE,
    [("4..", [SrcInstall])]
);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::pkg::BuildablePackage;
    use crate::shell::{test::FileTree, BuildData};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as default_src_install;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(default_src_install, &[1]);
    }

    #[test]
    fn valid_phase() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing default_src_install command"
            SLOT=0
            VAR=1
            DOCS=( "${FILESDIR}"/readme )
            src_install() {
                default_src_install
                VAR=2
            }
        "#};
        let pkg = t.create_pkg_from_str("cat/pkg-1", data).unwrap();

        // create docs file
        let filesdir = pkg.abspath().parent().unwrap().join("files");
        fs::create_dir(&filesdir).unwrap();
        fs::write(filesdir.join("readme"), "data").unwrap();

        BuildData::from_pkg(&pkg);
        let file_tree = FileTree::new();
        pkg.build().unwrap();
        // verify default src_install() was run
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkg-1/readme"
        "#,
        );
        // verify custom src_install() was run
        assert_eq!(scallop::variables::optional("VAR").as_deref(), Some("2"));
    }

    #[test]
    fn invalid_phase() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing default_src_install command"
            SLOT=0
            VAR=1
            pkg_setup() {
                default_src_install
                VAR=2
            }
        "#};
        let pkg = t.create_pkg_from_str("cat/pkg-1", data).unwrap();
        BuildData::from_pkg(&pkg);
        let r = pkg.build();
        assert_err_re!(r, "default_src_install: disabled in pkg_setup scope$");
        // verify custom pkg_setup() stopped on error
        assert_eq!(scallop::variables::optional("VAR").as_deref(), Some("1"));
    }
}
