use std::path::PathBuf;

use once_cell::sync::Lazy;
use relative_path::RelativePath;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::install::create_link;
use crate::pkgsh::BUILD_DATA;

static LONG_DOC: &str = "Create symbolic links.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let (source, target) = match args.len() {
            3 if args[0] == "-r" && eapi.has("dosym_relative") => {
                let (source, target) = (PathBuf::from(args[1]), PathBuf::from(args[2]));
                if !source.is_absolute() {
                    return Err(Error::Builtin(format!(
                        "`dosym -r` requires absolute source: {:?}",
                        source
                    )));
                }
                let mut parent = PathBuf::from("/");
                if let Some(p) = target.parent() {
                    parent.push(p)
                }
                let relpath = RelativePath::from_path(&parent)
                    .map_err(|e| Error::Builtin(format!("invalid relative path: {}", e)))?;
                (relpath.to_logical_path(source), target)
            }
            2 => (PathBuf::from(args[0]), PathBuf::from(args[1])),
            n => return Err(Error::Builtin(format!("requires 2 args, got {}", n))),
        };

        // check for unsupported dir target arg -- https://bugs.gentoo.org/379899
        if target.file_name().is_none() || (target.is_dir() && !target.is_symlink()) {
            return Err(Error::Builtin(format!("missing filename target: {:?}", target)));
        }

        create_link(false, source, target)?;

        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "dosym",
            func: run,
            help: LONG_DOC,
            usage: "dosym path/to/source /path/to/target",
        },
        &[("0-", &["src_install"])],
    )
});

#[cfg(test)]
mod tests {
    use super::super::assert_invalid_args;
    use super::run as dosym;
    use crate::eapi::OFFICIAL_EAPIS;
    use crate::pkgsh::BUILD_DATA;

    use rusty_fork::rusty_fork_test;

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(dosym, &[0, 1, 4]);

            BUILD_DATA.with(|d| {
                for eapi in OFFICIAL_EAPIS.values().filter(|e| !e.has("dosym_relative")) {
                    d.borrow_mut().eapi = eapi;
                    assert_invalid_args(dosym, &[3]);
                }
            });
        }
    }
}
