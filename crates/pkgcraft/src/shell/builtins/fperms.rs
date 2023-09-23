use std::process::Command;

use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::command::RunCommand;
use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::{PkgPostinst, PkgPreinst, SrcInstall};

use super::make_builtin;

const LONG_DOC: &str = "Run `chmod` taking paths relative to the image directory.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.len() < 2 {
        return Err(Error::Base(format!("requires at least 2 args, got {}", args.len())));
    }

    Command::new("chmod")
        .args(args.iter().map(|s| s.trim_start_matches('/')))
        .current_dir(get_build_mut().destdir())
        .run_with_output()?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "fperms mode /path/to/file";
make_builtin!(
    "fperms",
    fperms_builtin,
    run,
    LONG_DOC,
    USAGE,
    [("..", [SrcInstall, PkgPreinst, PkgPostinst])]
);

#[cfg(test)]
mod tests {
    use crate::command::run_commands;
    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::pkg::BuildPackage;
    use crate::shell::{test::FileTree, BuildData};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as fperms;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(fperms, &[0, 1]);
    }

    #[test]
    fn failure() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let data = indoc::formatdoc! {r#"
            EAPI=8
            DESCRIPTION="testing fperms command"
            SLOT=0
            src_install() {{
                fperms 0777 /nonexistent
            }}
        "#};
        let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
        BuildData::from_pkg(&pkg);
        let _file_tree = FileTree::new();
        run_commands(|| {
            let r = pkg.build();
            assert_err_re!(
                r,
                "failed running: chmod: cannot access 'nonexistent': No such file or directory$"
            );
        })
    }

    #[test]
    fn success() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        for eapi in BUILTIN.scope.keys() {
            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="testing fperms command"
                SLOT=0
                src_install() {{
                    touch file1 file2
                    doins file1 file2
                    # absolute paths work
                    fperms 0777 /file1
                    # relative paths work
                    fperms 0757 file2
                }}
            "#};
            let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
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
