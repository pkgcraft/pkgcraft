use std::process::Command;

use scallop::{Error, ExecStatus};

use crate::command::RunCommand;
use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Run `chown` taking paths relative to the image directory.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.len() < 2 {
        return Err(Error::Base(format!("requires at least 2 args, got {}", args.len())));
    }

    Command::new("chown")
        .args(args.iter().map(|s| s.trim_start_matches('/')))
        .current_dir(get_build_mut().destdir())
        .run_with_output()?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "fowners user:group /path/to/file";
make_builtin!("fowners", fowners_builtin);

#[cfg(test)]
mod tests {
    use crate::command::{commands, run_commands};
    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::pkg::Build;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::{test::FileTree, BuildData};
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_args, cmd_scope_tests, fowners};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(fowners, &[0, 1]);
    }

    #[test]
    fn failure() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        let data = indoc::formatdoc! {r#"
            EAPI=8
            DESCRIPTION="testing fowners command"
            SLOT=0
            src_install() {{
                fowners nonexistent:nonexistent /nonexistent
            }}
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        let _file_tree = FileTree::new();
        run_commands(|| {
            let r = pkg.build();
            assert_err_re!(r, "failed running: chown: .*$");
        })
    }

    #[test]
    fn success() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="testing fowners command"
                SLOT=0
                src_install() {{
                    touch file1 file2
                    doins file1 file2
                    # absolute paths work
                    fowners 0:0 /file1
                    # relative paths work
                    fowners root:root file2
                }}
            "#};
            temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
            let pkg = repo.get_pkg("cat/pkg-1").unwrap();
            BuildData::from_pkg(&pkg);
            let _file_tree = FileTree::new();
            // fake running fowners since modifying file ownership requires elevated permissions
            pkg.build().unwrap();
            let cmds = commands();
            assert_eq!(&cmds, &[["chown", "0:0", "file1"], ["chown", "root:root", "file2"]]);
        }
    }
}
