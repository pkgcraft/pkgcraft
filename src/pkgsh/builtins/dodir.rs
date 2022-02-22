use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Install directories.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let opts = &d.borrow().diropts;
        let install = d.borrow().install().dir_options(opts);
        install.dirs(args)?;
        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "dodir",
            func: run,
            help: LONG_DOC,
            usage: "dodir path/to/dir",
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
    use super::super::diropts::run as diropts;
    use super::run as dodir;
    use crate::pkgsh::BUILD_DATA;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(dodir, &[0]);
        }

        #[test]
        fn creation() {
            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                env::set_current_dir(&dir).unwrap();
                let prefix = dir.path();
                d.borrow_mut().env.insert("ED".into(), prefix.to_str().unwrap().into());

                let default = 0o40755;

                for dirs in [
                        vec!["dir"],
                        vec!["path/to/dir"],
                        vec!["/etc"],
                        vec!["/usr/bin"],
                        vec!["dir", "/usr/bin"],
                        ] {
                    dodir(&dirs).unwrap();
                    for dir in dirs {
                        let path = Path::new(dir.strip_prefix("/").unwrap_or(dir));
                        let path: PathBuf = [prefix, path].iter().collect();
                        assert!(path.is_dir(), "failed creating dir: {dir:?}");
                        let meta = fs::metadata(&path).unwrap();
                        let mode = meta.mode();
                        assert!(mode == default, "mode {mode:#o} is not default {default:#o}");
                    }
                }
            })
        }

        #[test]
        fn custom_diropts() {
            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                env::set_current_dir(&dir).unwrap();
                let prefix = dir.path();
                d.borrow_mut().env.insert("ED".into(), prefix.to_str().unwrap().into());

                let default = 0o40755;
                let custom = 0o40777;

                for dir in ["dir", "/usr/bin"] {
                    diropts(&["-m0755"]).unwrap();
                    dodir(&[dir]).unwrap();
                    let path = Path::new(dir.strip_prefix("/").unwrap_or(dir));
                    let path: PathBuf = [prefix, path].iter().collect();
                    let meta = fs::metadata(&path).unwrap();
                    let mode = meta.mode();
                    assert!(mode == default, "mode {mode:#o} is not default {default:#o}");

                    // change mode and re-run dodir()
                    diropts(&["-m0777"]).unwrap();
                    dodir(&[dir]).unwrap();
                    let meta = fs::metadata(&path).unwrap();
                    let mode = meta.mode();
                    assert!(mode == custom, "mode {mode:#o} is not custom {custom:#o}");
                }
            })
        }
    }
}
