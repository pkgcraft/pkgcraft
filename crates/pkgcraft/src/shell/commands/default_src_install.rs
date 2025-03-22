use scallop::ExecStatus;

use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "default_src_install",
    long_about = indoc::indoc! {"
        Runs the default src_install implementation for a package's EAPI.
    "}
)]
struct Command;

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let _cmd = Command::try_parse_args(args)?;
    get_build_mut().phase().default()
}

make_builtin!("default_src_install", default_src_install_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::Config;
    use crate::pkg::Build;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::{test::FileTree, BuildData};
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, default_src_install};

    cmd_scope_tests!("default_src_install");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(default_src_install, &[1]);
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
            DESCRIPTION="testing default_src_install command"
            SLOT=0
            VAR=1
            DOCS=( "${FILESDIR}"/readme )
            src_install() {
                default_src_install
                VAR=2
            }
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();

        // create docs file
        let filesdir = pkg.path().parent().unwrap().join("files");
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
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

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
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        let r = pkg.build();
        assert_err_re!(r, "line 6: default_src_install: error: disabled in pkg_setup scope$");
        // verify custom pkg_setup() stopped on error
        assert_eq!(scallop::variables::optional("VAR").as_deref(), Some("1"));
    }
}
