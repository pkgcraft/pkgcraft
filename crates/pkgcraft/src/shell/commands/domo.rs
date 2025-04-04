use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use scallop::ExecStatus;

use crate::eapi::Feature::DomoUsesDesttree;
use crate::macros::build_path;
use crate::shell::environment::Variable::DESTTREE;
use crate::shell::get_build_mut;

use super::{make_builtin, TryParseArgs};

#[derive(clap::Parser, Debug)]
#[command(
    name = "domo",
    disable_help_flag = true,
    long_about = "Install gettext *.mo files."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(required = true, allow_hyphen_values = true, value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let build = get_build_mut();
    let dest = if build.eapi().has(DomoUsesDesttree) {
        build.env(DESTTREE)
    } else {
        "/usr"
    };
    let opts = ["-m0644"];
    let install = build
        .install()
        .dest(build_path!(dest, "share/locale"))?
        .file_options(opts);

    let mut dirs = IndexSet::new();
    let mut files = vec![];
    let filename = format!("{}.mo", build.cpv().package());

    for path in cmd.paths {
        if let Some(dir) = path.file_stem().map(Utf8Path::new) {
            let dir = dir.join("LC_MESSAGES");
            files.push((path, dir.join(&filename)));
            dirs.insert(dir);
        }
    }

    install.dirs(dirs)?;
    install.files_map(files)?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "domo path/to/mo/file";
make_builtin!("domo", domo_builtin, true);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::test::FileTree;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, domo, into};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(domo, &[0]);

        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = domo(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        // verify DESTTREE is used depending on EAPI
        for eapi in &*EAPIS_OFFICIAL {
            temp.create_ebuild("cat/pkg-1", &[&format!("EAPI={eapi}")])
                .unwrap();
            let pkg = repo.get_pkg("cat/pkg-1").unwrap();
            BuildData::from_pkg(&pkg);
            let file_tree = FileTree::new();
            fs::File::create("en.mo").unwrap();
            into(&["opt"]).unwrap();
            domo(&["en.mo"]).unwrap();
            let path = if eapi.has(DomoUsesDesttree) {
                "/opt/share/locale/en/LC_MESSAGES/pkg.mo"
            } else {
                "/usr/share/locale/en/LC_MESSAGES/pkg.mo"
            };
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "{path}"
                mode = 0o100644
            "#
            ));
        }
    }
}
