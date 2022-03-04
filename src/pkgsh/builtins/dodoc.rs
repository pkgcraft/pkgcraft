use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install documentation files.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let (recursive, args) = match args.first() {
            Some(&"-r") if eapi.has("dodoc_recursive") => (true, &args[1..]),
            _ => (false, args),
        };

        if args.is_empty() {
            return Err(Error::Builtin("requires 1 or more targets, got 0".into()));
        }

        let dest: PathBuf = [
            "/usr/share/doc",
            d.borrow().env.get("PF").expect("$PF undefined"),
            &d.borrow().docdesttree,
        ]
        .iter()
        .collect();
        let install = d.borrow().install().dest(&dest)?;

        let (dirs, files): (Vec<&Path>, Vec<&Path>) =
            args.iter().map(Path::new).partition(|p| p.is_dir());

        if !dirs.is_empty() {
            if recursive {
                install.from_dirs(dirs)?;
            } else {
                return Err(Error::Builtin(format!(
                    "trying to install directory as file: {:?}",
                    dirs[0]
                )));
            }
        }

        let files = files
            .into_iter()
            .filter_map(|f| f.file_name().map(|name| (f, name)));
        install.files(files)?;

        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "dodoc",
            func: run,
            help: LONG_DOC,
            usage: "dodoc [-r] doc_file",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::fs;

    use rusty_fork::rusty_fork_test;

    use super::super::assert_invalid_args;
    use super::super::docinto::run as docinto;
    use super::run as dodoc;
    use crate::macros::assert_err_re;
    use crate::pkgsh::test::FileTree;
    use crate::pkgsh::BUILD_DATA;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(dodoc, &[0]);

            BUILD_DATA.with(|d| d.borrow_mut().env.insert("PF".into(), "pkgcraft-0".into()));
            let _file_tree = FileTree::new();

            // nonexistent
            let r = dodoc(&["pkgcraft"]);
            assert_err_re!(r, format!("^invalid file \"pkgcraft\": .*$"));

            // non-recursive directory
            fs::create_dir("dir").unwrap();
            let r = dodoc(&["dir"]);
            assert_err_re!(r, format!("^trying to install directory as file: .*$"));
        }

        #[test]
        fn creation() {
            BUILD_DATA.with(|d| d.borrow_mut().env.insert("PF".into(), "pkgcraft-0".into()));
            let file_tree = FileTree::new();
            let default_mode: u32 = 0o100644;

            // simple file
            fs::File::create("file").unwrap();
            file_tree.assert(
                || {
                    dodoc(&["file"]).unwrap();
                },
                format!(
                    r#"
                    [[files]]
                    path = "/usr/share/doc/pkgcraft-0/file"
                    mode = {default_mode}
                    "#
                ),
            );

            // recursive using `docinto`
            fs::create_dir_all("doc/subdir").unwrap();
            fs::File::create("doc/subdir/file").unwrap();
            file_tree.assert(
                || {
                    docinto(&["newdir"]).unwrap();
                    dodoc(&["-r", "doc"]).unwrap();
                },
                r#"
                [[files]]
                path = "/usr/share/doc/pkgcraft-0/newdir/doc/subdir/file"
                "#,
            );

            // handling for paths ending in '/.'
            file_tree.assert(
                || {
                    dodoc(&["-r", "doc/."]).unwrap();
                },
                r#"
                [[files]]
                path = "/usr/share/doc/pkgcraft-0/newdir/subdir/file"
                "#,
            );
        }
    }
}
