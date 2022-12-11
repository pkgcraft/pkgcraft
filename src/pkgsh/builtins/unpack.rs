use std::ops::BitOr;

use camino::{Utf8Path, Utf8PathBuf};
use nix::sys::stat::{fchmodat, lstat, FchmodatFlags::FollowSymlink, Mode, SFlag};
use scallop::builtins::ExecStatus;
use scallop::{variables, Error};
use walkdir::WalkDir;

use crate::archive::ArchiveFormat;
use crate::eapi::Feature;
use crate::pkgsh::BUILD_DATA;
use crate::utils::current_dir;

use super::{make_builtin, PHASE};

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
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let current_dir = current_dir()?;

    BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
        let d = d.borrow();
        let eapi = d.eapi;
        let distdir = variables::required("DISTDIR")?;

        // Determine the source for a given archive target. Basic filenames are prefixed with
        // DISTDIR while all other types are unprefixed including conditionally supported absolute
        // and relative paths.
        let determine_source = |path: &Utf8Path| -> scallop::Result<Utf8PathBuf> {
            if path.parent() == Some(Utf8Path::new("")) {
                Ok(Utf8PathBuf::from(&distdir).join(path))
            } else if path.starts_with("./") || eapi.has(Feature::UnpackExtendedPath) {
                Ok(Utf8PathBuf::from(path))
            } else {
                let adj = match path.is_absolute() {
                    true => "absolute",
                    false => "relative",
                };
                let err = format!("{adj} paths not supported in EAPI {eapi}: {path:?}");
                Err(Error::Base(err))
            }
        };

        for path in args.iter().map(Utf8Path::new) {
            let source = determine_source(path)?;
            if !source.exists() {
                return Err(Error::Base(format!("nonexistent archive: {path}")));
            }

            let (ext, archive) = eapi.archive_from_path(&source)?;
            let base = source.file_name().unwrap();
            let base = &base[0..base.len() - 1 - ext.len()];
            let dest = &current_dir.join(base);
            archive.unpack(dest)?;
        }

        // ensure proper permissions on unpacked files with minimal syscalls
        for entry in WalkDir::new(&current_dir).min_depth(1) {
            let entry =
                entry.map_err(|e| Error::Base(format!("failed walking {current_dir:?}: {e}")))?;
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

const USAGE: &str = "unpack file.tar.gz";
make_builtin!("unpack", unpack_builtin, run, LONG_DOC, USAGE, &[("..", &[PHASE])]);

#[cfg(test)]
mod tests {
    use std::ops::BitXor;
    use std::{env, fs};

    use nix::sys::stat::{fchmodat, lstat, FchmodatFlags::FollowSymlink, Mode};
    use scallop::variables::bind;
    use tempfile::tempdir;

    use crate::archive::{Archive, ArchiveFormat};
    use crate::command::run_commands;
    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::pkgsh::BUILD_DATA;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::*;
    use super::{run as unpack, DIR_MODE, FILE_MODE};

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(unpack, &[0]);
    }

    #[test]
    fn nonexistent() {
        bind("DISTDIR", "dist", None, None).unwrap();
        assert_err_re!(unpack(&["a.tar.gz"]), "^nonexistent archive: .*$");
    }

    #[test]
    fn eapi_features() {
        BUILD_DATA.with(|d| {
            let tmp_dir = tempdir().unwrap();
            let prefix = tmp_dir.path();
            let distdir = prefix.join("distdir");
            fs::create_dir(&distdir).unwrap();
            env::set_current_dir(prefix).unwrap();
            bind("DISTDIR", distdir.to_str().unwrap(), None, None).unwrap();
            fs::File::create("distdir/a.TAR.GZ").unwrap();
            let abs_path = prefix.join("distdir/a.tar.gz");
            fs::File::create(&abs_path).unwrap();

            for eapi in EAPIS_OFFICIAL.iter() {
                d.borrow_mut().eapi = eapi;

                // case insensitive support
                let result = unpack(&["a.TAR.GZ"]);
                if eapi.has(Feature::UnpackCaseInsensitive) {
                    result.unwrap();
                } else {
                    assert_err_re!(result, "^unknown archive format: .*$");
                }

                // absolute path support
                let result = unpack(&[abs_path.to_str().unwrap()]);
                if eapi.has(Feature::UnpackExtendedPath) {
                    result.unwrap();
                } else {
                    assert_err_re!(result, "^absolute paths not supported .*$");
                }

                // prefixed relative paths work everywhere
                unpack(&["./distdir/a.tar.gz"]).unwrap();

                // unprefixed are EAPI conditional
                let result = unpack(&["distdir/a.tar.gz"]);
                if eapi.has(Feature::UnpackExtendedPath) {
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
        let tmp_dir = tempdir().unwrap();
        let prefix = tmp_dir.path();
        let datadir = prefix.join("data");
        let distdir = prefix.join("distdir");
        fs::create_dir(&distdir).unwrap();
        env::set_current_dir(prefix).unwrap();
        bind("DISTDIR", distdir.to_str().unwrap(), None, None).unwrap();

        // create archive source
        let dir = datadir.join("dir");
        let file = dir.join("file");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&file, "pkgcraft").unwrap();

        // disable permissions that should get reset during unpack
        fchmodat(None, &dir, DIR_MODE.bitxor(Mode::S_IXOTH), FollowSymlink).unwrap();
        fchmodat(None, &file, FILE_MODE.bitxor(Mode::S_IWUSR), FollowSymlink).unwrap();

        // compressed archives
        for ext in ["tar.gz", "tar.bz2", "tar.xz"] {
            // create tarball, remove its source, and then unpack it
            run_commands(|| {
                let file = format!("a.{ext}");
                let path = distdir.join(&file);
                env::set_current_dir(&datadir).unwrap();
                Archive::pack("dir", path.to_str().unwrap()).unwrap();
                env::set_current_dir(prefix).unwrap();
                unpack(&[&file]).unwrap();
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

            // remove unpacked data
            fs::remove_dir_all("dir").unwrap();
        }

        // compressed files
        for ext in ["gz", "bz2", "xz"] {
            // create archive, remove its source, and then unpack it
            run_commands(|| {
                let file = format!("file.{ext}");
                let path = distdir.join(&file);
                env::set_current_dir(&dir).unwrap();
                Archive::pack("file", path.to_str().unwrap()).unwrap();
                env::set_current_dir(prefix).unwrap();
                unpack(&[&file]).unwrap();
            });

            // verify unpacked data
            assert_eq!(fs::read_to_string("file").unwrap(), "pkgcraft");

            // verify permissions got reset
            let stat = lstat("file").unwrap();
            let mode = Mode::from_bits_truncate(stat.st_mode);
            assert!(mode.contains(*FILE_MODE), "incorrect file mode: {mode:#o}");

            // remove unpacked data
            fs::remove_file("file").unwrap();
        }
    }
}
