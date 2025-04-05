use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fmt, fs, io};

use clap::Parser;
use filetime::{set_file_times, FileTime};
use indexmap::IndexMap;
use itertools::Either;
use nix::{fcntl::AtFlags, sys::stat, unistd};
use rayon::prelude::*;
use scallop::Error;
use walkdir::{DirEntry, WalkDir};

use crate::command::RunCommand;
use crate::files::{Group, Mode, User};

use super::BuildData;

#[derive(Parser, Debug, Default)]
#[clap(name = "install")]
struct InstallOptions {
    #[clap(short, long)]
    group: Option<Group>,
    #[clap(short, long)]
    owner: Option<User>,
    #[clap(short, long)]
    mode: Option<Mode>,
    #[clap(short, long)]
    preserve_timestamps: bool,
}

enum InstallOpts {
    Internal(InstallOptions),
    Cmd(Vec<String>),
}

#[derive(Default)]
pub(super) struct Install {
    destdir: PathBuf,
    file_options: Option<InstallOpts>,
    dir_options: Option<InstallOpts>,
}

impl Install {
    pub(super) fn new(build: &BuildData) -> Self {
        Install {
            destdir: PathBuf::from(build.destdir()),
            ..Default::default()
        }
    }

    /// Set the target directory to install files into.
    pub(super) fn dest<P: AsRef<Path>>(mut self, dest: P) -> scallop::Result<Self> {
        let dest = dest.as_ref();
        self.dirs([dest])?;
        self.destdir.push(dest.strip_prefix("/").unwrap_or(dest));
        Ok(self)
    }

    fn parse_options<I>(&self, options: I) -> Option<InstallOpts>
    where
        I: IntoIterator,
        I::Item: fmt::Display,
    {
        let options: Vec<_> = options.into_iter().map(|s| s.to_string()).collect();
        if options.is_empty() {
            None
        } else {
            let cmd = ["install"]
                .into_iter()
                .chain(options.iter().map(|s| s.as_str()));

            match InstallOptions::try_parse_from(cmd) {
                Ok(opts) => Some(InstallOpts::Internal(opts)),
                Err(_) => Some(InstallOpts::Cmd(options)),
            }
        }
    }

    /// Parse options to use for file attributes during install.
    pub(super) fn file_options<I>(mut self, options: I) -> Self
    where
        I: IntoIterator,
        I::Item: fmt::Display,
    {
        self.file_options = self.parse_options(options);
        self
    }

    /// Parse options to use for dir attributes during install.
    pub(super) fn dir_options<I>(mut self, options: I) -> Self
    where
        I: IntoIterator,
        I::Item: fmt::Display,
    {
        self.dir_options = self.parse_options(options);
        self
    }

    /// Prefix a given path with the target directory.
    pub(super) fn prefix<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        let path = path.as_ref();
        self.destdir.join(path.strip_prefix("/").unwrap_or(path))
    }

    /// Create a soft or hardlink between a given source and target.
    pub(super) fn link<F, P, Q>(&self, link: F, source: P, target: Q) -> scallop::Result<()>
    where
        F: Fn(&Path, &Path) -> io::Result<()>,
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let (source, target) = (source.as_ref(), target.as_ref());

        // create parent dirs
        if let Some(parent) = target.parent() {
            self.dirs([parent])?;
        }

        // capture target value before it's prefixed
        let failed = |e: io::Error| -> Error {
            Error::Base(format!("failed creating link: {source:?} -> {target:?}: {e}"))
        };

        let target = self.prefix(target);

        // overwrite link if it exists
        while let Err(e) = link(source, &target) {
            if e.kind() == io::ErrorKind::AlreadyExists {
                fs::remove_file(&target).map_err(failed)?;
            } else {
                return Err(failed(e));
            }
        }

        Ok(())
    }

    /// Set the attributes of a file.
    fn set_attributes<P: AsRef<Path>>(
        &self,
        opts: &InstallOptions,
        path: P,
    ) -> scallop::Result<()> {
        let path = path.as_ref();
        let uid = opts.owner.as_ref().map(|o| o.uid);
        let gid = opts.group.as_ref().map(|g| g.gid);
        if uid.is_some() || gid.is_some() {
            unistd::fchownat(None, path, uid, gid, AtFlags::AT_SYMLINK_NOFOLLOW).map_err(
                |e| Error::Base(format!("failed setting file uid/gid: {path:?}: {e}")),
            )?;
        }

        if let Some(mode) = &opts.mode {
            if !path.is_symlink() {
                stat::fchmodat(None, path, **mode, stat::FchmodatFlags::FollowSymlink)
                    .map_err(|e| {
                        Error::Base(format!("failed setting file mode: {path:?}: {e}"))
                    })?;
            }
        }

        Ok(())
    }

    /// Create given directories under the target directory.
    pub(super) fn dirs<I>(&self, paths: I) -> scallop::Result<()>
    where
        I: IntoIterator + IntoParallelIterator,
        <I as IntoIterator>::Item: AsRef<Path>,
        <I as IntoParallelIterator>::Item: AsRef<Path>,
    {
        if let Some(InstallOpts::Cmd(opts)) = &self.dir_options {
            self.dirs_cmd(opts, paths)
        } else {
            self.dirs_internal(paths)
        }
    }

    /// Create directories in parallel using internal functionality.
    fn dirs_internal<I>(&self, paths: I) -> scallop::Result<()>
    where
        I: IntoParallelIterator,
        I::Item: AsRef<Path>,
    {
        paths
            .into_par_iter()
            .try_for_each(|path| -> scallop::Result<()> {
                let path = self.prefix(path);
                fs::create_dir_all(&path)
                    .map_err(|e| Error::Base(format!("failed creating dir: {path:?}: {e}")))?;
                if let Some(InstallOpts::Internal(opts)) = &self.dir_options {
                    self.set_attributes(opts, path)?;
                }
                Ok(())
            })
    }

    /// Create directories using the `install` command.
    fn dirs_cmd<I>(&self, opts: &[String], paths: I) -> scallop::Result<()>
    where
        I: IntoIterator,
        I::Item: AsRef<Path>,
    {
        let mut install = Command::new("install");
        install
            .args(opts)
            .arg("-d")
            .args(paths.into_iter().map(|p| self.prefix(p)))
            .run()
            .map_err(|e| Error::Base(e.to_string()))
    }

    /// Copy file trees under given paths to the target directory.
    pub(super) fn recursive<I, F>(&self, paths: I, predicate: Option<F>) -> scallop::Result<()>
    where
        I: IntoIterator,
        I::Item: AsRef<Path>,
        F: Fn(&DirEntry) -> bool,
    {
        let mut dirs = vec![];
        let mut files = vec![];

        for path in paths {
            let path = path.as_ref();
            // Determine whether to skip the base directory, path.components() can't be used
            // because it normalizes all occurrences of '.' away.
            let depth = if path.to_string_lossy().ends_with("/.") {
                1
            } else {
                0
            };

            // optionally apply directory filtering
            let entries = WalkDir::new(path).min_depth(depth);
            let entries = match predicate.as_ref() {
                None => Either::Left(entries.into_iter()),
                Some(func) => Either::Right(entries.into_iter().filter_entry(func)),
            };

            for entry in entries {
                let entry =
                    entry.map_err(|e| Error::Base(format!("error walking {path:?}: {e}")))?;
                let path = entry.path();
                // TODO: replace with advance_by() once it's stable
                let dest = match depth {
                    0 => path,
                    n => {
                        let mut comp = path.components();
                        for _ in 0..n {
                            comp.next();
                        }
                        comp.as_path()
                    }
                };
                if path.is_dir() {
                    dirs.push(dest.to_path_buf());
                } else {
                    files.push((path.to_path_buf(), dest.to_path_buf()));
                }
            }
        }

        self.dirs(dirs)?;
        self.files_map(files)?;

        Ok(())
    }

    /// Install files from their given paths to the target directory.
    pub(super) fn files<'a, I, P>(&self, paths: I) -> scallop::Result<()>
    where
        I: IntoIterator<Item = &'a P>,
        P: AsRef<Path> + 'a + ?Sized,
    {
        let files: Vec<_> = paths
            .into_iter()
            .map(|p| p.as_ref())
            .filter_map(|p| p.file_name().map(|name| (p, name)))
            .collect();

        if let Some(InstallOpts::Cmd(opts)) = &self.file_options {
            self.files_cmd(opts, files)
        } else {
            self.files_internal(files)
        }
    }

    /// Install files using a custom source -> dest mapping to the target directory.
    pub(super) fn files_map<I, P, Q>(&self, paths: I) -> scallop::Result<()>
    where
        I: IntoIterator<Item = (P, Q)>,
        I: IntoParallelIterator<Item = (P, Q)>,
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        if let Some(InstallOpts::Cmd(opts)) = &self.file_options {
            self.files_cmd(opts, paths)
        } else {
            self.files_internal(paths)
        }
    }

    // Install files using internal functionality.
    fn files_internal<I, P, Q>(&self, paths: I) -> scallop::Result<()>
    where
        I: IntoParallelIterator<Item = (P, Q)>,
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        paths
            .into_par_iter()
            .try_for_each(|(source, dest)| -> scallop::Result<()> {
                let source = source.as_ref();
                let dest = self.prefix(dest.as_ref());
                let meta = fs::metadata(source)
                    .map_err(|e| Error::Base(format!("invalid file {source:?}: {e}")))?;

                // matching `install` command, remove dest before install
                match fs::remove_file(&dest) {
                    Err(e) if e.kind() != io::ErrorKind::NotFound => {
                        return Err(Error::Base(format!(
                            "failed removing file: {dest:?}: {e}"
                        )));
                    }
                    _ => (),
                }

                fs::copy(source, &dest).map_err(|e| {
                    Error::Base(format!("failed copying file: {source:?} to {dest:?}: {e}"))
                })?;
                if let Some(InstallOpts::Internal(opts)) = &self.file_options {
                    self.set_attributes(opts, &dest)?;
                    if opts.preserve_timestamps {
                        let atime = FileTime::from_last_access_time(&meta);
                        let mtime = FileTime::from_last_modification_time(&meta);
                        set_file_times(&dest, atime, mtime).map_err(|e| {
                            Error::Base(format!("failed setting file time: {e}"))
                        })?;
                    }
                }

                Ok(())
            })
    }

    // Install files using the `install` command.
    fn files_cmd<I, P, Q>(&self, opts: &[String], paths: I) -> scallop::Result<()>
    where
        I: IntoIterator<Item = (P, Q)>,
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let mut files = IndexMap::<_, Vec<_>>::new();

        for (source, dest) in paths {
            let source = source.as_ref();
            let dest = dest.as_ref();
            if let Ok(source) = fs::read_link(source) {
                // install symlinks separately since `install` forcibly resolves them
                self.link(|p, q| symlink(p, q), source, dest)?;
            } else {
                // group files by destination to decrease `install` calls
                files
                    .entry(self.prefix(dest))
                    .or_default()
                    .push(source.to_path_buf());
            }
        }

        files
            .into_par_iter()
            .try_for_each(|(dest, sources)| -> scallop::Result<()> {
                let mut install = Command::new("install");
                install
                    .args(opts)
                    .args(sources)
                    .arg(dest)
                    .run()
                    .map_err(|e| Error::Base(e.to_string()))
            })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::command::{commands, run_commands};
    use crate::shell::get_build_mut;
    use crate::shell::test::FileTree;
    use crate::test::assert_err_re;

    #[test]
    fn nonexistent() {
        let _file_tree = FileTree::new();
        let r = get_build_mut()
            .install()
            .files_internal([("source", "dest")]);
        assert_err_re!(r, "^invalid file \"source\": No such file or directory .*$");
    }

    #[test]
    fn dirs() {
        let file_tree = FileTree::new();

        // internal dir creation is used for supported `install` options
        let install = get_build_mut().install().dir_options(["-m0750"]);
        let mode = 0o40750;

        // single dir
        install.dirs(["dir"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/dir"
            mode = {mode}
        "#
        ));

        // multiple dirs
        install.dirs(["a", "b"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/a"
            mode = {mode}
            [[files]]
            path = "/b"
            mode = {mode}
        "#,
        ));

        // use unhandled '-v' option to force `install` command usage
        let install = get_build_mut().install().dir_options(["-v"]);
        let default_mode = 0o40755;

        // single dir
        install.dirs(["dir"]).unwrap();
        let cmd = commands().pop().unwrap();
        assert_eq!(cmd[..3], ["install", "-v", "-d"]);
        run_commands(|| {
            install.dirs(["dir"]).unwrap();
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/dir"
                mode = {default_mode}
            "#
            ));
        });

        // multiple dirs
        install.dirs(["a", "b"]).unwrap();
        let cmd = commands().pop().unwrap();
        assert_eq!(cmd[..3], ["install", "-v", "-d"]);
        run_commands(|| {
            install.dirs(["a", "b"]).unwrap();
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/a"
                mode = {default_mode}
                [[files]]
                path = "/b"
                mode = {default_mode}
            "#
            ));
        });
    }

    #[test]
    fn files() {
        let file_tree = FileTree::new();
        // internal file creation is used for supported `install` options
        let install = get_build_mut().install().file_options(["-m0750"]);
        let mode = 0o100750;
        fs::File::create("file1").unwrap();
        fs::File::create("file2").unwrap();

        // single file
        install.files(["file1"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/file1"
            mode = {mode}
        "#
        ));

        // multiple files
        install.files(["file1", "file2"]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/file1"
            mode = {mode}
            [[files]]
            path = "/file2"
            mode = {mode}
        "#
        ));

        // single file mapping
        install.files_map([("file1", "dest")]).unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/dest"
            mode = {mode}
        "#
        ));

        // multiple file mapping
        install
            .files_map([("file1", "dest1"), ("file2", "dest2")])
            .unwrap();
        file_tree.assert(format!(
            r#"
            [[files]]
            path = "/dest1"
            mode = {mode}
            [[files]]
            path = "/dest2"
            mode = {mode}
        "#
        ));

        // use unhandled '-v' option to force `install` command usage
        let install = get_build_mut().install().file_options(["-v"]);
        let default_mode = 0o100755;

        // single file target
        install.files(["file1"]).unwrap();
        let cmd = commands().pop().unwrap();
        // verify `install` command
        assert_eq!(cmd[..3], ["install", "-v", "file1"]);
        // verify installed files
        run_commands(|| {
            install.files(["file1"]).unwrap();
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/file1"
                mode = {default_mode}
            "#
            ));
        });

        // single file mapping
        install.files_map([("file1", "dest1")]).unwrap();
        let cmd = commands().pop().unwrap();
        // verify `install` command
        assert_eq!(cmd[..3], ["install", "-v", "file1"]);
        // verify installed files
        run_commands(|| {
            install.files_map([("file1", "dest1")]).unwrap();
            file_tree.assert(format!(
                r#"
                [[files]]
                path = "/dest1"
                mode = {default_mode}
            "#
            ));
        });
    }
}
