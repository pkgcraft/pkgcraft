use std::fs::File;

use camino::Utf8PathBuf;
use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::{TryParseArgs, make_builtin};

#[derive(clap::Parser, Debug)]
#[command(
    name = "keepdir",
    disable_help_flag = true,
    long_about = indoc::indoc! {"
        For each argument, creates a directory as for dodir, and an empty file whose name
        starts with .keep in that directory to ensure that the directory does not get
        removed by the package manager should it be empty at any point.
    "}
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(required = true, allow_hyphen_values = true, value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,
}

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let build = get_build_mut();
    let install = build.install();

    // use custom file name including pkg info
    let pkg = build.ebuild_pkg();
    let (cat, pkg, slot) = (pkg.cpv().category(), pkg.cpv().package(), pkg.slot());
    let file_name = format!(".keep_{cat}_{pkg}_{slot}");

    // create dirs
    install.dirs(&cmd.paths)?;

    // create stub files
    for path in cmd.paths {
        let keep = install.prefix(path).join(&file_name);
        File::create(&keep)
            .map_err(|e| Error::Base(format!("failed creating keep file: {keep:?}: {e}")))?;
    }

    Ok(ExecStatus::Success)
}

make_builtin!("keepdir", keepdir_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::pkg::Build;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::BuildData;
    use crate::shell::test::FileTree;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, functions::keepdir};

    cmd_scope_tests!("keepdir path/to/dir");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(keepdir, &[0]);
    }

    #[test]
    fn creation() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();

        for dirs in [
            vec!["dir"],
            vec!["path/to/dir"],
            vec!["/etc"],
            vec!["/usr/bin"],
            vec!["dir", "/usr/bin"],
            vec!["-"],
        ] {
            let args = dirs.join(" ");
            let data = indoc::formatdoc! {r#"
                EAPI=8
                DESCRIPTION="testing keepdir"
                SLOT=0
                S=${{WORKDIR}}
                src_install() {{
                    keepdir {args}
                }}
            "#};
            temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
            let pkg = repo.get_pkg("cat/pkg-1").unwrap();
            BuildData::from_pkg(&pkg);
            let file_tree = FileTree::new();
            pkg.build().unwrap();

            let mut files = vec![];
            for dir in dirs {
                let path = dir.trim_start_matches('/');
                files.push(format!(
                    r#"
                    [[files]]
                    path = "/{path}/.keep_cat_pkg_0"
                    mode = 0o100644
                    data = ""
                "#
                ));
            }
            file_tree.assert(files.join("\n"));
        }
    }
}
