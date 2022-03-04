use std::path::Path;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install files into INSDESTREE.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let (recursive, args) = match args.first() {
        Some(&"-r") => (true, &args[1..]),
        _ => (false, args),
    };

    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more targets, got 0".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let dest = &d.borrow().insdesttree;
        let opts = &d.borrow().insopts;
        let install = d.borrow().install().dest(&dest)?.ins_options(opts);

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
            name: "doins",
            func: run,
            help: LONG_DOC,
            usage: "doins [-r] path/to/file",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::fs;

    use rusty_fork::rusty_fork_test;

    use super::super::assert_invalid_args;
    use super::super::insinto::run as insinto;
    use super::super::insopts::run as insopts;
    use super::run as doins;
    use crate::macros::assert_err_re;
    use crate::pkgsh::test::FileTree;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(doins, &[0]);

            let _file_tree = FileTree::new();

            // nonexistent
            let r = doins(&["pkgcraft"]);
            assert_err_re!(r, format!("^invalid file \"pkgcraft\": .*$"));

            // non-recursive directory
            fs::create_dir("dir").unwrap();
            let r = doins(&["dir"]);
            assert_err_re!(r, format!("^trying to install directory as file: .*$"));
        }

        #[test]
        fn creation() {
            let file_tree = FileTree::new();

            let default_mode = 0o100644;
            let custom_mode = 0o100755;

            // simple file
            fs::File::create("file").unwrap();
            file_tree.assert(
                || {
                    doins(&["file"]).unwrap();
                },
                format!(
                    r#"
                    [[files]]
                    path = "/file"
                    mode = {default_mode}
                    "#
                ),
            );

            // recursive using `insinto` and `insopts`
            fs::create_dir_all("dir/subdir").unwrap();
            fs::File::create("dir/subdir/file").unwrap();
            file_tree.assert(
                || {
                    insinto(&["newdir"]).unwrap();
                    insopts(&["-m0755"]).unwrap();
                    doins(&["-r", "dir"]).unwrap();
                },
                format!(
                    r#"
                    [[files]]
                    path = "/newdir/dir/subdir/file"
                    mode = {custom_mode}
                    "#
                ),
            );
        }
    }
}
