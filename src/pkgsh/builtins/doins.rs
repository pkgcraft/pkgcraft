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
    use std::os::unix::fs::MetadataExt;
    use std::path::{Path, PathBuf};
    use std::{env, fs};

    use rusty_fork::rusty_fork_test;
    use tempfile::tempdir;

    use super::super::assert_invalid_args;
    use super::super::insinto::run as insinto;
    use super::super::insopts::run as insopts;
    use super::run as doins;
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(doins, &[0]);

            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                env::set_current_dir(&dir).unwrap();
                let prefix = dir.path();
                d.borrow_mut().env.insert("ED".into(), prefix.to_str().unwrap().into());

                // nonexistent
                let r = doins(&["pkgcraft"]);
                assert_err_re!(r, format!("^invalid file \"pkgcraft\": .*$"));

                // non-recursive directory
                fs::create_dir("dir").unwrap();
                let r = doins(&["dir"]);
                assert_err_re!(r, format!("^trying to install directory as file: .*$"));
            })
        }

        #[test]
        fn creation() {
            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                let prefix = dir.path();
                let src_dir = prefix.join("src");
                fs::create_dir(&src_dir).unwrap();
                env::set_current_dir(&src_dir).unwrap();
                d.borrow_mut().env.insert("ED".into(), prefix.to_str().unwrap().into());

                let default = 0o100644;
                let custom = 0o100755;

                // simple file
                fs::File::create("file").unwrap();
                doins(&["file"]).unwrap();
                let path = Path::new("file");
                let path: PathBuf = [prefix, path].iter().collect();
                let meta = fs::metadata(&path).unwrap();
                let mode = meta.mode();
                assert!(mode == default, "mode {mode:#o} is not default {default:#o}");

                // recursive using `insinto` and `insopts`
                fs::create_dir_all("dir/subdir").unwrap();
                fs::File::create("dir/subdir/file").unwrap();
                insinto(&["newdir"]).unwrap();
                insopts(&["-m0755"]).unwrap();
                doins(&["-r", "dir"]).unwrap();
                let path = Path::new("newdir/dir/subdir/file");
                let path: PathBuf = [prefix, path].iter().collect();
                let meta = fs::metadata(&path).unwrap();
                let mode = meta.mode();
                assert!(mode == custom, "mode {mode:#o} is not custom {custom:#o}");
            })
        }
    }
}
