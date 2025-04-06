use std::ops::BitOr;
use std::str::FromStr;
use std::sync::LazyLock;

use camino::{Utf8Path, Utf8PathBuf};
use nix::sys::stat::{fchmodat, lstat, FchmodatFlags::FollowSymlink, Mode, SFlag};
use rayon::prelude::*;
use scallop::{Error, ExecStatus};
use walkdir::WalkDir;

use crate::archive::ArchiveFormat;
use crate::eapi::Feature;
use crate::shell::environment::Variable::DISTDIR;
use crate::shell::get_build_mut;
use crate::utils::is_single_component;

use super::{make_builtin, TryParseArgs};

// TODO: Drop LazyLock usage once upstream BitOr is marked const (see
// https://github.com/bitflags/bitflags/issues/180) requiring const trait impl support in
// rust (see https://github.com/rust-lang/rust/issues/67792).
//
// unpacked file required permissions: a+r,u+w,go-w
static FILE_MODE: LazyLock<Mode> = LazyLock::new(|| {
    Mode::S_IRUSR
        | Mode::S_IRGRP
        | Mode::S_IROTH
        | Mode::S_IWUSR & !Mode::S_IWGRP & !Mode::S_IWOTH
});
// unpacked dir required permissions: a+rx,u+w,go-w
static DIR_MODE: LazyLock<Mode> =
    LazyLock::new(|| *FILE_MODE | Mode::S_IXUSR | Mode::S_IXGRP | Mode::S_IXOTH);

#[derive(Debug, Clone)]
struct Archive(Utf8PathBuf);

impl FromStr for Archive {
    type Err = scallop::Error;

    fn from_str(s: &str) -> scallop::Result<Self> {
        let build = get_build_mut();
        let eapi = build.eapi();
        let distdir = build.env(DISTDIR);
        let path = Utf8Path::new(s);

        // Determine the source for a given archive target. Basic filenames are prefixed with
        // DISTDIR while all other types are unprefixed including conditionally supported absolute
        // and relative paths.
        let source = if is_single_component(path) {
            Utf8Path::new(distdir).join(path)
        } else if path.starts_with("./") || eapi.has(Feature::UnpackExtendedPath) {
            Utf8PathBuf::from(path)
        } else {
            let kind = if path.is_absolute() {
                "absolute"
            } else {
                "relative"
            };
            return Err(Error::Base(format!("EAPI {eapi}: unsupported {kind} path: {path}")));
        };

        if !source.exists() {
            return Err(Error::Base(format!("nonexistent archive: {path}")));
        }

        Ok(Self(source))
    }
}

#[derive(clap::Parser, Debug)]
#[command(
    name = "unpack",
    disable_help_flag = true,
    long_about = "Unpacks one or more source archives, in order, into the current directory."
)]
struct Command {
    #[arg(long, action = clap::ArgAction::HelpLong)]
    help: Option<bool>,

    #[arg(required = true, allow_hyphen_values = true, value_name = "PATH")]
    paths: Vec<Archive>,
}

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let cmd = Command::try_parse_args(args)?;
    let build = get_build_mut();
    let eapi = build.eapi();

    // unpack all specified archives
    for path in cmd.paths {
        let (ext, archive) = eapi.archive_from_path(&path.0)?;
        let base = path.0.file_name().expect("invalid archive file name");
        let base = &base[0..base.len() - 1 - ext.len()];
        archive.unpack(base)?;
    }

    // TODO: parallelize walking fs
    // gather all unpacked files
    let entries: Vec<_> = WalkDir::new(".").min_depth(1).into_iter().collect();

    // ensure proper permissions on unpacked files with minimal syscalls in parallel
    entries
        .into_par_iter()
        .try_for_each(|entry| -> scallop::Result<()> {
            let entry = entry.map_err(|e| Error::Base(format!("failed walking fs: {e}")))?;
            let path = entry.path();
            let stat = lstat(path).map_err(|e| {
                Error::Base(format!(
                    "failed getting file status: {}: {e}",
                    path.to_string_lossy()
                ))
            })?;
            let mode = Mode::from_bits_truncate(stat.st_mode);

            // alter file permissions if necessary
            if let Some(new_mode) = match SFlag::from_bits_truncate(stat.st_mode) {
                flags if flags.contains(SFlag::S_IFLNK) => None,
                flags if flags.contains(SFlag::S_IFDIR) => {
                    if !mode.contains(*DIR_MODE) {
                        Some(mode.bitor(*DIR_MODE))
                    } else {
                        None
                    }
                }
                _ => {
                    if !mode.contains(*FILE_MODE) {
                        Some(mode.bitor(*FILE_MODE))
                    } else {
                        None
                    }
                }
            } {
                fchmodat(None, path, new_mode, FollowSymlink).map_err(|e| {
                    Error::Base(format!(
                        "failed changing permissions: {}: {e}",
                        path.to_string_lossy()
                    ))
                })?;
            }

            Ok(())
        })?;

    Ok(ExecStatus::Success)
}

make_builtin!("unpack", unpack_builtin, true);

#[cfg(test)]
mod tests {
    use std::ops::BitXor;
    use std::{env, fs};

    use nix::sys::stat::{fchmodat, lstat, FchmodatFlags::FollowSymlink, Mode};
    use tempfile::tempdir;

    use crate::archive::{Archive, ArchiveFormat};
    use crate::command::run_commands;
    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::shell::BuildData;
    use crate::test::assert_err_re;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, unpack};
    use super::*;

    cmd_scope_tests!("unpack file.tar.gz");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(unpack, &[0]);
    }

    #[test]
    fn nonexistent() {
        let build = get_build_mut();
        build.env.insert(DISTDIR, "dist".to_string());
        assert_err_re!(unpack(&["a.tar.gz"]), "nonexistent archive: a.tar.gz");
    }

    #[test]
    fn eapi_features() {
        let build = get_build_mut();
        let tmp_dir = tempdir().unwrap();
        let prefix = tmp_dir.path();
        let distdir = prefix.join("distdir");
        fs::create_dir(&distdir).unwrap();
        env::set_current_dir(prefix).unwrap();
        build
            .env
            .insert(DISTDIR, distdir.to_str().unwrap().to_string());
        fs::File::create("distdir/a.TAR.GZ").unwrap();
        let abs_path = prefix.join("distdir/a.tar.gz");
        fs::File::create(&abs_path).unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            BuildData::empty(eapi);

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
                assert_err_re!(result, format!("EAPI {eapi}: unsupported absolute path: .*"));
            }

            // prefixed relative paths work everywhere
            unpack(&["./distdir/a.tar.gz"]).unwrap();

            // unprefixed are EAPI conditional
            let result = unpack(&["distdir/a.tar.gz"]);
            if eapi.has(Feature::UnpackExtendedPath) {
                result.unwrap();
            } else {
                assert_err_re!(result, format!("EAPI {eapi}: unsupported relative path: .*"));
            }
        }
    }

    #[test]
    #[cfg_attr(target_os = "macos", ignore)] // TODO: switch to builtin support?
    fn archives() {
        let build = get_build_mut();
        let tmp_dir = tempdir().unwrap();
        let prefix = tmp_dir.path();
        let datadir = prefix.join("data");
        let distdir = prefix.join("distdir");
        fs::create_dir(&distdir).unwrap();
        env::set_current_dir(prefix).unwrap();
        build
            .env
            .insert(DISTDIR, distdir.to_str().unwrap().to_string());

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
