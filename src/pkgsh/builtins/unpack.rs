use std::env;
use std::ops::BitOr;

use camino::{Utf8Path, Utf8PathBuf};
use nix::sys::stat::{fchmodat, lstat, FchmodatFlags::NoFollowSymlink, Mode, SFlag};
use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};
use walkdir::WalkDir;

use super::{PkgBuiltin, PHASE};
use crate::pkgsh::archive::{Archive, Compression};
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "\
Unpacks one or more source archives, in order, into the current directory.";

static FILE_MODE: Lazy<Mode> = Lazy::new(|| {
    Mode::S_IRUSR | Mode::S_IRGRP | Mode::S_IROTH | Mode::S_IWUSR & !Mode::S_IWGRP & !Mode::S_IWOTH
});
static DIR_MODE: Lazy<Mode> =
    Lazy::new(|| *FILE_MODE | Mode::S_IXUSR | Mode::S_IXGRP | Mode::S_IXOTH);

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    let dir =
        env::current_dir().map_err(|e| Error::Builtin(format!("can't get current dir: {e}")))?;
    let dir = Utf8PathBuf::try_from(dir)
        .map_err(|e| Error::Builtin(format!("invalid unicode path: {e}")))?;

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let d = d.borrow();
        let eapi = d.eapi;
        let distdir = d.env.get("DISTDIR").expect("$DISTDIR undefined");

        for path in args.iter().map(Utf8Path::new) {
            let determine_srcdir = || -> Result<Utf8PathBuf> {
                if path.parent() == Some(Utf8Path::new("")) {
                    Ok(Utf8PathBuf::from(distdir))
                } else if path.starts_with("./") || eapi.has("unpack_absolute_paths") {
                    Ok(Utf8PathBuf::from(""))
                } else if path.is_absolute() {
                    return Err(Error::Builtin(format!("absolute paths not supported: {path:?}")));
                } else {
                    return Err(Error::Builtin(format!(
                        "relative paths require './' prefix in EAPI {eapi}: {path:?}"
                    )));
                }
            };

            let srcdir = determine_srcdir()?;
            let source = srcdir.join(path);

            if !source.exists() {
                return Err(Error::Builtin(format!("nonexistent archive: {path}")));
            }

            let (ext, archive) = Archive::from_path(&source, eapi)?;
            let base = source.file_name().unwrap();
            let base = &base[0..base.len() - 1 - ext.len()];
            let dest = &dir.join(base);
            archive.unpack(dest)?;
        }

        // ensure proper permissions on unpacked files with minimal syscalls
        for entry in WalkDir::new(&dir).min_depth(1) {
            let entry = entry.map_err(|e| Error::Base(format!("failed walking {dir:?}: {e}")))?;
            let path = entry.path();
            let stat =
                lstat(path).map_err(|e| Error::Base(format!("failed file stat {path:?}: {e}")))?;
            let current_mode = Mode::from_bits_truncate(stat.st_mode);
            let mode = match stat.st_mode {
                mode if (mode & SFlag::S_IFLNK.bits() == 1) => continue,
                mode if (mode & SFlag::S_IFDIR.bits() == 1) => current_mode.bitor(*DIR_MODE),
                _ => current_mode.bitor(*FILE_MODE),
            };
            fchmodat(None, path, mode, NoFollowSymlink)
                .map_err(|e| Error::Base(format!("failed file chmod {path:?}: {e}")))?;
        }

        Ok(ExecStatus::Success)
    })
}

pub(super) static BUILTIN: Lazy<PkgBuiltin> = Lazy::new(|| {
    PkgBuiltin::new(
        Builtin {
            name: "unpack",
            func: run,
            help: LONG_DOC,
            usage: "unpack file.tar.gz",
        },
        &[("0-", &[PHASE])],
    )
});

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use rusty_fork::rusty_fork_test;
    use tempfile::tempdir;

    use super::super::assert_invalid_args;
    use super::run as unpack;
    use crate::eapi::OFFICIAL_EAPIS;
    use crate::macros::assert_err_re;
    use crate::pkgsh::archive::{Archive, Compression};
    use crate::pkgsh::{run_commands, BUILD_DATA};

    rusty_fork_test! {
        #[test]
        fn invalid_args() {
            assert_invalid_args(unpack, &[0]);
        }

        #[test]
        fn nonexistent() {
            BUILD_DATA.with(|d| {
                d.borrow_mut().env.insert("DISTDIR".into(), "dist".into());
                assert_err_re!(unpack(&["a.tar.gz"]), "^nonexistent archive: .*$");
            })
        }

        #[test]
        fn case_insensitive() {
            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                let prefix = dir.path();
                let dist = prefix.join("dist");
                fs::create_dir(&dist).unwrap();
                env::set_current_dir(&prefix).unwrap();
                d.borrow_mut().env.insert("DISTDIR".into(), dist.to_str().unwrap().into());
                fs::File::create("dist/a.TAR.GZ").unwrap();

                for eapi in OFFICIAL_EAPIS.values() {
                    d.borrow_mut().eapi = eapi;
                    if eapi.has("unpack_case_insensitive") {
                        unpack(&["a.TAR.GZ"]).unwrap();
                    } else {
                        assert_err_re!(unpack(&["a.TAR.GZ"]), "^unknown archive format: .*$");
                    }
                }
            })
        }

        #[test]
        fn tar_gz() {
            BUILD_DATA.with(|d| {
                let dir = tempdir().unwrap();
                let prefix = dir.path();
                let dist = prefix.join("dist");
                fs::create_dir(&dist).unwrap();
                env::set_current_dir(&prefix).unwrap();
                d.borrow_mut().env.insert("DISTDIR".into(), dist.to_str().unwrap().into());

                // create archive source
                let tar = prefix.join("tar");
                fs::create_dir(&tar).unwrap();
                fs::write("tar/data", "pkgcraft").unwrap();

                // create archive, remove its source, and then unpack it
                run_commands(|| {
                    Archive::pack("tar", "a.tar.gz").unwrap();
                    fs::remove_dir_all("tar").unwrap();
                    unpack(&["./a.tar.gz"]).unwrap();
                });

                // verify unpacked data
                assert_eq!(fs::read_to_string("tar/data").unwrap(), "pkgcraft");
            })
        }
    }
}
