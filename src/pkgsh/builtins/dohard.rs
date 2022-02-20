use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Create hard links.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    let (source, target) = match args.len() {
        2 => (args[0], args[1]),
        n => return Err(Error::Builtin(format!("requires 2 args, got {n}"))),
    };

    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        d.borrow().create_link(true, source, target)?;
        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "dohard",
            func: run,
            help: LONG_DOC,
            usage: "dohard path/to/source /path/to/target",
        },
        &[("0-3", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::os::unix::fs::MetadataExt;
    use std::{env, fs};

    use rusty_fork::rusty_fork_test;
    use tempfile::tempdir;

    use super::super::assert_invalid_args;
    use super::run as dohard;
    use crate::pkgsh::BUILD_DATA;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(dohard, &[0, 1, 3]);
        }

        #[test]
        fn linking() {
            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                env::set_current_dir(&dir).unwrap();
                let path = dir.path().to_str().unwrap();
                d.borrow_mut().env.insert("ED".into(), path.into());
                let source = dir.path().join("source");
                File::create(source).unwrap();

                dohard(&["source", "target"]).unwrap();
                let source_meta = fs::metadata("source").unwrap();
                let target_meta = fs::metadata("target").unwrap();
                // hard link inodes match
                assert_eq!(source_meta.ino(), target_meta.ino());
            })
        }
    }
}
