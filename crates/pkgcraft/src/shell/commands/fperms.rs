use scallop::ExecStatus;

use crate::command::RunCommand;
use crate::shell::get_build_mut;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "fperms",
    disable_help_flag = true,
    long_about = "Run `chmod` taking paths relative to the image directory."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    // can't easily split options from arguments without listing all supported options
    #[arg(required = true, allow_hyphen_values = true, num_args = 2..)]
    args: Vec<String>,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;

    std::process::Command::new("chmod")
        .args(cmd.args.iter().map(|s| s.trim_start_matches('/')))
        .current_dir(get_build_mut().destdir())
        .run_with_output()?;

    Ok(ExecStatus::Success)
}

make_builtin!("fperms", fperms_builtin);

#[cfg(test)]
mod tests {
    use crate::command::run_commands;
    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::pkg::Build;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::{BuildData, test::FileTree};
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, functions::fperms};

    cmd_scope_tests!("fperms mode /path/to/file");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(fperms, &[0, 1]);
    }

    #[test]
    fn failure() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        let data = indoc::formatdoc! {r#"
            EAPI=8
            DESCRIPTION="testing fperms command"
            SLOT=0
            S=${{WORKDIR}}
            src_install() {{
                fperms 0777 /nonexistent
            }}
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        let _file_tree = FileTree::new();
        run_commands(|| {
            let r = pkg.build();
            assert_err_re!(r, "line 6: fperms: error: failed running: chmod: .+$");
        })
    }

    #[test]
    fn success() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="testing fperms command"
                SLOT=0
                S=${{WORKDIR}}
                src_install() {{
                    touch file1 file2
                    doins file1 file2
                    # absolute paths work
                    fperms 0777 /file1
                    # relative paths work
                    fperms 0757 file2
                }}
            "#};
            temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
            let pkg = repo.get_pkg("cat/pkg-1").unwrap();
            BuildData::from_pkg(&pkg);
            let file_tree = FileTree::new();
            run_commands(|| pkg.build().unwrap());
            // verify file modes were changed
            file_tree.assert(
                r#"
                [[files]]
                path = "/file1"
                mode = 0o100777
                [[files]]
                path = "/file2"
                mode = 0o100757
            "#,
            );
        }
    }
}
