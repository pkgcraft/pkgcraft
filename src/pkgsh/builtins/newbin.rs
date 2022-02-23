use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::Result;

use super::_new::new;
use super::dobin::run as dobin;
use super::PkgBuiltin;

static LONG_DOC: &str = "Install renamed executables into DESTTREE/bin.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    new(args, dobin)
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "newbin",
            func: run,
            help: LONG_DOC,
            usage: "newbin path/to/executable filename",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::unix::fs::MetadataExt;
    use std::path::{Path, PathBuf};
    use std::{env, fs};

    use rusty_fork::rusty_fork_test;
    use tempfile::tempdir;

    use super::super::assert_invalid_args;
    use super::run as newbin;
    use crate::pkgsh::{write_stdin, BUILD_DATA};

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(newbin, &[0, 1, 3]);
        }

        #[test]
        fn creation_from_file() {
            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                let prefix = dir.path();
                let src_dir = prefix.join("src");
                fs::create_dir(&src_dir).unwrap();
                env::set_current_dir(&src_dir).unwrap();
                d.borrow_mut().env.insert("ED".into(), prefix.to_str().unwrap().into());

                let default = 0o100755;

                fs::File::create("bin").unwrap();
                newbin(&["bin", "pkgcraft"]).unwrap();
                let path = Path::new("usr/bin/pkgcraft");
                let path: PathBuf = [prefix, path].iter().collect();
                assert!(path.is_file(), "failed creating file: {path:?}");
                let meta = fs::metadata(&path).unwrap();
                let mode = meta.mode();
                assert!(mode == default, "mode {mode:#o} is not default {default:#o}");
            })
        }

        #[test]
        fn creation_from_stdin() {
            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                let prefix = dir.path();
                let src_dir = prefix.join("src");
                fs::create_dir(&src_dir).unwrap();
                env::set_current_dir(&src_dir).unwrap();
                d.borrow_mut().env.insert("ED".into(), prefix.to_str().unwrap().into());

                write_stdin!("pkgcraft");
                newbin(&["-", "pkgcraft"]).unwrap();
                let path = Path::new("usr/bin/pkgcraft");
                let path: PathBuf = [prefix, path].iter().collect();
                assert!(path.is_file(), "failed creating file: {path:?}");
                assert_eq!(fs::read_to_string(&path).unwrap(), "pkgcraft");
            })
        }
    }
}
