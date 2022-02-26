use std::fs::File;

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "Install directories.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let install = d.borrow().install();
        // create dirs
        install.dirs(args)?;
        // create stub files
        for path in args {
            // TODO: add pkg data to file name
            let keep = install.prefix(path).join(".keep");
            File::create(&keep)
                .map_err(|e| Error::Builtin(format!("failed creating keep file: {keep:?}: {e}")))?;
        }
        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "keepdir",
            func: run,
            help: LONG_DOC,
            usage: "keepdir path/to/kept/dir",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::{Path, PathBuf};

    use rusty_fork::rusty_fork_test;
    use tempfile::tempdir;

    use super::super::assert_invalid_args;
    use super::run as keepdir;
    use crate::pkgsh::BUILD_DATA;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(keepdir, &[0]);
        }

        #[test]
        fn creation() {
            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                env::set_current_dir(&dir).unwrap();
                let prefix = dir.path();
                d.borrow_mut().env.insert("ED".into(), prefix.to_str().unwrap().into());

                for dirs in [
                        vec!["dir"],
                        vec!["path/to/dir"],
                        vec!["/etc"],
                        vec!["/usr/bin"],
                        vec!["dir", "/usr/bin"],
                        ] {
                    keepdir(&dirs).unwrap();
                    for dir in dirs {
                        let path = Path::new(dir.strip_prefix("/").unwrap_or(dir));
                        let path: PathBuf = [prefix, path].iter().collect();
                        assert!(path.is_dir(), "failed creating dir: {dir:?}");
                        assert!(path.join(".keep").is_file());
                    }
                }
            })
        }
    }
}
