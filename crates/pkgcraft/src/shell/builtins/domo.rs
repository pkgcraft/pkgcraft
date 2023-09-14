use std::collections::HashSet;
use std::path::{Path, PathBuf};

use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::macros::build_from_paths;
use crate::pkg::Package;
use crate::shell::environment::VariableKind::DESTTREE;
use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind::SrcInstall;

use super::{make_builtin, Scopes::Phase};

const LONG_DOC: &str = "Install gettext *.mo files.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let build = get_build_mut();
    let dest = build_from_paths!(
        build
            .env
            .get(&DESTTREE)
            .map(|s| s.as_str())
            .unwrap_or("/usr"),
        "share/locale"
    );
    let opts = ["-m0644"];
    let install = build.install().dest(dest)?.file_options(opts);

    let (mut dirs, mut files) = (HashSet::<PathBuf>::new(), Vec::<(&Path, PathBuf)>::new());
    let filename = format!("{}.mo", build.pkg()?.cpv().package());

    for path in args.iter().map(Path::new) {
        let dir = match path.file_stem() {
            None => continue,
            Some(v) => Path::new(v).join("LC_MESSAGES"),
        };
        files.push((path, dir.join(&filename)));
        dirs.insert(dir);
    }

    install.dirs(dirs)?;
    install.files_map(files)?;

    Ok(ExecStatus::Success)
}

const USAGE: &str = "domo path/to/mo/file";
make_builtin!("domo", domo_builtin, run, LONG_DOC, USAGE, &[("..", &[Phase(SrcInstall)])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::Config;
    use crate::shell::test::FileTree;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as domo;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(domo, &[0]);
    }

    #[test]
    fn creation() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_pkg(&pkg);

        let file_tree = FileTree::new();
        let default_mode = 0o100644;

        fs::File::create("en.mo").unwrap();
        domo(&["en.mo"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/usr/share/locale/en/LC_MESSAGES/pkg.mo"
            mode = {default_mode}
        "#
        ));
    }
}
