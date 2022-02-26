use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};

use super::PkgBuiltin;
use crate::pkgsh::BUILD_DATA;
use crate::utils::relpath;

const LONG_DOC: &str = "Create symbolic links.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let eapi = d.borrow().eapi;
        let (source, target, target_str) = match args.len() {
            3 if args[0] == "-r" && eapi.has("dosym_relative") => {
                let (source, target) = (Path::new(args[1]), Path::new(args[2]));
                if !source.is_absolute() {
                    return Err(Error::Builtin(format!(
                        "absolute source required with '-r': {source:?}",
                    )));
                }
                let mut parent = PathBuf::from("/");
                if let Some(p) = target.parent() {
                    parent.push(p)
                }
                let source = relpath(&source, &parent).ok_or_else(|| {
                    Error::Builtin(format!("invalid relative path: {source:?} -> {target:?}"))
                })?;
                (source, target, args[2])
            }
            2 => (PathBuf::from(args[0]), Path::new(args[1]), args[1]),
            n => return Err(Error::Builtin(format!("requires 2 args, got {n}"))),
        };

        // check for unsupported dir target arg -- https://bugs.gentoo.org/379899
        if target_str.ends_with('/') || (target.is_dir() && !target.is_symlink()) {
            return Err(Error::Builtin(format!("missing filename target: {target:?}")));
        }

        let install = d.borrow().install();
        install.link(|p, q| symlink(p, q), source, target)?;

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
    use std::path::Path;
    use std::{env, fs};

    use rusty_fork::rusty_fork_test;
    use tempfile::tempdir;

    use super::super::assert_invalid_args;
    use super::run as dosym;
    use crate::eapi::OFFICIAL_EAPIS;
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

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

        #[test]
        fn errors() {
            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                env::set_current_dir(&dir).unwrap();
                let path = dir.path().to_str().unwrap();
                d.borrow_mut().env.insert("ED".into(), path.into());

                // dir targets aren't supported
                let r = dosym(&["source", "target/"]);
                assert_err_re!(r, format!("^missing filename target: .*$"));

                fs::create_dir("target").unwrap();
                let r = dosym(&["source", "target"]);
                assert_err_re!(r, format!("^missing filename target: .*$"));

                // relative source with `dosym -r`
                let r = dosym(&["-r", "source", "target"]);
                assert_err_re!(r, format!("^absolute source required .*$"));
            })
        }

        #[test]
        fn linking() {
            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                env::set_current_dir(&dir).unwrap();
                let path = dir.path().to_str().unwrap();
                d.borrow_mut().env.insert("ED".into(), path.into());

                dosym(&["/usr/bin/source", "target"]).unwrap();
                assert_eq!(Path::new("/usr/bin/source"), fs::read_link("./target").unwrap());
                dosym(&["-r", "/usr/bin/source", "/usr/bin/target"]).unwrap();
                assert_eq!(Path::new("source"), fs::read_link("./usr/bin/target").unwrap());
            })
        }
    }
}
