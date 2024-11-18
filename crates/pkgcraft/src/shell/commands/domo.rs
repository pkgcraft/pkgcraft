use std::collections::HashSet;

use camino::Utf8Path;
use scallop::{Error, ExecStatus};

use crate::macros::build_path;
use crate::shell::environment::Variable::DESTTREE;
use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "Install gettext *.mo files.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let build = get_build_mut();
    let dest = build_path!(
        build
            .env
            .get(&DESTTREE)
            .map(|s| s.as_str())
            .unwrap_or("/usr"),
        "share/locale"
    );
    let opts = ["-m0644"];
    let install = build.install().dest(dest)?.file_options(opts);

    let mut dirs = HashSet::new();
    let mut files = vec![];
    let filename = format!("{}.mo", build.cpv().package());

    for path in args.iter().map(Utf8Path::new) {
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
make_builtin!("domo", domo_builtin);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::shell::test::FileTree;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::TEST_DATA;

    use super::super::{assert_invalid_args, cmd_scope_tests, domo};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(domo, &[0]);

        let repo = TEST_DATA.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let _file_tree = FileTree::new();

        // nonexistent
        let r = domo(&["nonexistent"]);
        assert_err_re!(r, "^invalid file \"nonexistent\": No such file or directory .*$");
    }

    #[test]
    fn creation() {
        let repo = TEST_DATA.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();

        fs::File::create("en.mo").unwrap();
        domo(&["en.mo"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/locale/en/LC_MESSAGES/pkg.mo"
            mode = 0o100644
        "#,
        );
    }
}
