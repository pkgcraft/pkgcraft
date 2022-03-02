use std::env;
use std::ops::BitOr;

use camino::{Utf8Path, Utf8PathBuf};
use nix::sys::stat::{fchmodat, lstat, FchmodatFlags::FollowSymlink, Mode, SFlag};
use once_cell::sync::Lazy;
use scallop::builtins::{Builtin, ExecStatus};
use scallop::{Error, Result};
use walkdir::WalkDir;

use super::{PkgBuiltin, PHASE};
use crate::pkgsh::archive::{Archive, Compression};
use crate::pkgsh::BUILD_DATA;

const LONG_DOC: &str = "\
Unpacks one or more source archives, in order, into the current directory.";

// unpacked file required permissions: a+r,u+w,go-w
static FILE_MODE: Lazy<Mode> = Lazy::new(|| {
    Mode::S_IRUSR | Mode::S_IRGRP | Mode::S_IROTH | Mode::S_IWUSR & !Mode::S_IWGRP & !Mode::S_IWOTH
});
// unpacked dir required permissions: a+rx,u+w,go-w
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

        let determine_srcdir = |path: &Utf8Path| -> Result<Utf8PathBuf> {
            if path.parent() == Some(Utf8Path::new("")) {
                // plain filename is prefixed with DISTDIR
                Ok(Utf8PathBuf::from(distdir))
            } else if path.starts_with("./") {
                // filenames starting with ./ are relative to the current dir
                Ok(Utf8PathBuf::from(""))
            } else {
                // absolute and relative path support is EAPI conditional
                if eapi.has("unpack_extended_path") {
                    Ok(Utf8PathBuf::from(""))
                } else {
                    let adj = match path.is_absolute() {
                        true => "absolute",
                        false => "relative",
                    };
                    let err = format!("{adj} paths not supported in EAPI {eapi}: {path:?}");
                    Err(Error::Builtin(err))
                }
            }
        };

        for path in args.iter().map(Utf8Path::new) {
            let srcdir = determine_srcdir(path)?;
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
            let mode = Mode::from_bits_truncate(stat.st_mode);
            let new_mode = match SFlag::from_bits_truncate(stat.st_mode) {
                flags if flags.contains(SFlag::S_IFLNK) => continue,
                flags if flags.contains(SFlag::S_IFDIR) => {
                    if !mode.contains(*DIR_MODE) {
                        mode.bitor(*DIR_MODE)
                    } else {
                        continue;
                    }
                }
                _ => {
                    if !mode.contains(*FILE_MODE) {
                        mode.bitor(*FILE_MODE)
                    } else {
                        continue;
                    }
                }
            };
            fchmodat(None, path, new_mode, FollowSymlink)
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
    use std::ops::BitXor;
    use std::{env, fs};

    use nix::sys::stat::{fchmodat, lstat, FchmodatFlags::FollowSymlink, Mode};
    use rusty_fork::rusty_fork_test;
    use tempfile::tempdir;

    use super::super::assert_invalid_args;
    use super::{run as unpack, DIR_MODE, FILE_MODE};
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
        fn eapi_features() {
            BUILD_DATA.with(|d| {
                let tmp_dir = tempdir().unwrap();
                let prefix = tmp_dir.path();
                let dist = prefix.join("dist");
                fs::create_dir(&dist).unwrap();
                env::set_current_dir(&prefix).unwrap();
                d.borrow_mut().env.insert("DISTDIR".into(), dist.to_str().unwrap().into());
                fs::File::create("dist/a.TAR.GZ").unwrap();
                let abs_path = prefix.join("dist/a.tar.gz");
                fs::File::create(&abs_path).unwrap();

                for eapi in OFFICIAL_EAPIS.values() {
                    d.borrow_mut().eapi = eapi;

                    // case insensitive support
                    let result = unpack(&["a.TAR.GZ"]);
                    if eapi.has("unpack_case_insensitive") {
                        result.unwrap();
                    } else {
                        assert_err_re!(result, "^unknown archive format: .*$");
                    }

                    // absolute path support
                    let result = unpack(&[abs_path.to_str().unwrap()]);
                    if eapi.has("unpack_extended_path") {
                        result.unwrap();
                    } else {
                        assert_err_re!(result, "^absolute paths not supported .*$");
                    }

                    // prefixed relative paths work everywhere
                    unpack(&["./dist/a.tar.gz"]).unwrap();

                    // unprefixed are EAPI conditional
                    let result = unpack(&["dist/a.tar.gz"]);
                    if eapi.has("unpack_extended_path") {
                        result.unwrap();
                    } else {
                        assert_err_re!(result, "^relative paths not supported .*$");
                    }
                }
            })
        }

        #[test]
        #[cfg_attr(target_os = "macos", ignore)] // TODO: switch to builtin support?
        fn archives() {
            BUILD_DATA.with(|d| {
                let tmp_dir = tempdir().unwrap();
                let prefix = tmp_dir.path();
                let datadir = prefix.join("data");
                let distdir = prefix.join("dist");
                fs::create_dir(&distdir).unwrap();
                env::set_current_dir(&prefix).unwrap();
                d.borrow_mut().env.insert("DISTDIR".into(), distdir.to_str().unwrap().into());

                // create archive source
                let dir = datadir.join("dir");
                let file = dir.join("file");
                fs::create_dir_all(&dir).unwrap();
                fs::write(&file, "pkgcraft").unwrap();

                // disable permissions that should get reset during unpack
                fchmodat(None, &dir, DIR_MODE.bitxor(Mode::S_IXOTH), FollowSymlink).unwrap();
                fchmodat(None, &file, FILE_MODE.bitxor(Mode::S_IWUSR), FollowSymlink).unwrap();

                for file in ["a.tar.gz", "a.tar.bz2", "a.tar.xz"] {
                    // create archive, remove its source, and then unpack it
                    run_commands(|| {
                        let archive_path = distdir.join(file);
                        env::set_current_dir(&datadir).unwrap();
                        Archive::pack("dir", archive_path.to_str().unwrap()).unwrap();
                        env::set_current_dir(&prefix).unwrap();
                        unpack(&[file]).unwrap();
                    });

                    // verify unpacked data
                    assert_eq!(fs::read_to_string("dir/file").unwrap(), "pkgcraft");

                    // verify permissions got reset
                    let stat = lstat("dir").unwrap();
                    let mode = Mode::from_bits_truncate(stat.st_mode);
                    assert!(mode.contains(*DIR_MODE), "incorrect dir mode: {mode:#o}");
                    let stat = lstat("dir/file").unwrap();
                    let mode = Mode::from_bits_truncate(stat.st_mode);
                    assert!(mode.contains(*FILE_MODE), "incorrect file mode: {mode:#o}");

                    // remove unpacked archive
                    fs::remove_dir_all("dir").unwrap();
                }
            })
        }
    }
}
