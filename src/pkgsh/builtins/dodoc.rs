use std::path::{Path, PathBuf};

use scallop::builtins::ExecStatus;
use scallop::{Error, Result};

use crate::eapi::Feature;
use crate::files::NO_WALKDIR_FILTER;
use crate::pkgsh::BUILD_DATA;

use super::make_builtin;

const LONG_DOC: &str = "Install documentation files.";

/// Install document files from a given iterable of paths.
pub(crate) fn install_docs<'a, I>(recursive: bool, paths: I) -> Result<ExecStatus>
where
    I: IntoIterator<Item = &'a Path>,
{
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let dest: PathBuf = [
            "/usr/share/doc",
            d.borrow().env.get("PF").expect("$PF undefined"),
            d.borrow().docdesttree.trim_start_matches('/'),
        ]
        .iter()
        .collect();
        let install = d.borrow().install().dest(&dest)?;

        let (dirs, files): (Vec<_>, Vec<_>) = paths.into_iter().partition(|p| p.is_dir());

        if !dirs.is_empty() {
            match recursive {
                true => install.recursive(dirs, NO_WALKDIR_FILTER),
                false => Err(Error::Base(format!("non-recursive dir install: {:?}", dirs[0]))),
            }?;
        }

        install.files(files)?;
        Ok(ExecStatus::Success)
    })
}

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let (recursive, args) = match args.first() {
            Some(&"-r") if eapi.has(Feature::DodocRecursive) => Ok((true, &args[1..])),
            Some(_) => Ok((false, args)),
            None => Err(Error::Base("requires 1 or more targets, got 0".into())),
        }?;

        install_docs(recursive, args.iter().map(Path::new))
    })
}

const USAGE: &str = "dodoc doc_file";
make_builtin!("dodoc", dodoc_builtin, run, LONG_DOC, USAGE, &[("0-", &["src_install"])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::macros::assert_err_re;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::BUILD_DATA;

    use super::super::docinto::run as docinto;
    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as dodoc;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(dodoc, &[0]);

        BUILD_DATA.with(|d| d.borrow_mut().env.insert("PF".into(), "pkgcraft-0".into()));
        let _file_tree = FileTree::new();

        // non-recursive directory
        fs::create_dir("dir").unwrap();
        let r = dodoc(&["dir"]);
        assert_err_re!(r, format!("^non-recursive dir install: .*$"));
    }

    #[test]
    fn creation() {
        BUILD_DATA.with(|d| d.borrow_mut().env.insert("PF".into(), "pkgcraft-0".into()));
        let file_tree = FileTree::new();
        let default_mode = 0o100644;

        // simple file
        fs::File::create("file").unwrap();
        dodoc(&["file"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/file"
            mode = {default_mode}
        "#
        ));

        // recursive using `docinto`
        fs::create_dir_all("doc/subdir").unwrap();
        fs::File::create("doc/subdir/file").unwrap();
        docinto(&["newdir"]).unwrap();
        dodoc(&["-r", "doc"]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/newdir/doc/subdir/file"
        "#,
        );

        // handling for paths ending in '/.'
        docinto(&["/newdir"]).unwrap();
        dodoc(&["-r", "doc/."]).unwrap();
        file_tree.assert(
            r#"
            [[files]]
            path = "/usr/share/doc/pkgcraft-0/newdir/subdir/file"
        "#,
        );
    }
}
