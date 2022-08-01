use std::collections::HashSet;
use std::path::{Path, PathBuf};

use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "Install gettext *.mo files.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let d = d.borrow();
        let dest: PathBuf =
            [d.env.get("DESTTREE").map(|s| s.as_str()).unwrap_or("/usr"), "share/locale"]
                .iter()
                .collect();
        let opts = ["-m0644"];
        let install = d.install().dest(&dest)?.file_options(opts);

        let (mut dirs, mut files) = (HashSet::<PathBuf>::new(), Vec::<(&Path, PathBuf)>::new());
        let filename = format!("{}.mo", d.env.get("PN").expect("$PN undefined"));

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
    })
}

const USAGE: &str = "domo path/to/mo/file";
make_builtin!("domo", domo_builtin, run, LONG_DOC, USAGE, &[("0-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::BUILD_DATA;

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
        BUILD_DATA.with(|d| d.borrow_mut().env.insert("PN".into(), "pkgcraft".into()));
        let file_tree = FileTree::new();
        let default_mode = 0o100644;

        fs::File::create("en.mo").unwrap();
        domo(&["en.mo"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/usr/share/locale/en/LC_MESSAGES/pkgcraft.mo"
            mode = {default_mode}
        "#
        ));
    }
}
