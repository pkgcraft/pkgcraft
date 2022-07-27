use std::collections::HashSet;
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install gettext *.mo files.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
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

make_builtin!("domo", domo_builtin, run, LONG_DOC, "domo path/to/mo/file");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &["src_install"])]));

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::assert_invalid_args;
    use super::run as domo;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::BUILD_DATA;

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
