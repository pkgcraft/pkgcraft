use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::{fs, io};

use clap::Parser;
use filetime::{set_file_times, FileTime};
use itertools::Itertools;
use nix::{sys::stat, unistd};
use scallop::{Error, Result};
use walkdir::WalkDir;

use super::BuildData;
use crate::command::RunCommand;

#[derive(Debug)]
struct Group {
    inner: unistd::Group,
}

impl FromStr for Group {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match unistd::Group::from_name(s) {
            Ok(Some(val)) => Ok(Group { inner: val }),
            Ok(None) => Err(crate::Error::InvalidValue(format!("unknown group: {s}"))),
            Err(_) => Err(crate::Error::InvalidValue(format!("invalid group: {s}"))),
        }
    }
}

#[derive(Debug)]
struct User {
    inner: unistd::User,
}

impl FromStr for User {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match unistd::User::from_name(s) {
            Ok(Some(val)) => Ok(User { inner: val }),
            Ok(None) => Err(crate::Error::InvalidValue(format!("unknown user: {s}"))),
            Err(_) => Err(crate::Error::InvalidValue(format!("invalid user: {s}"))),
        }
    }
}

#[derive(Debug)]
struct Mode {
    inner: stat::Mode,
}

impl FromStr for Mode {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let without_prefix = s.trim_start_matches("0o");
        let mode = stat::mode_t::from_str_radix(without_prefix, 8)
            .map_err(|_| crate::Error::InvalidValue(format!("invalid mode: {s}")))?;
        let mode = stat::Mode::from_bits(mode)
            .ok_or_else(|| crate::Error::InvalidValue(format!("invalid mode: {s}")))?;
        Ok(Mode { inner: mode })
    }
}

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
    // raw `install` options that were parsed
    raw: Vec<String>,
}

impl InstallOptions {
    fn gid(&self) -> Option<unistd::Gid> {
        self.group.as_ref().map(|g| g.inner.gid)
    }

    fn uid(&self) -> Option<unistd::Uid> {
        self.owner.as_ref().map(|o| o.inner.uid)
    }

    fn mode(&self) -> Option<stat::Mode> {
        self.mode.as_ref().map(|m| m.inner)
    }
}

#[derive(Default)]
pub(super) struct Install {
    destdir: PathBuf,
    file_options: Option<InstallOptions>,
    dir_options: Option<InstallOptions>,
    // fallback to using `install` due to unsupported options
    install_cmd: bool,
}

impl Install {
    pub(super) fn new(build: &BuildData) -> Self {
        let destdir = PathBuf::from(
            build
                .env
                .get("ED")
                .unwrap_or_else(|| build.env.get("D").expect("$D undefined")),
        );

        Install {
            destdir,
            ..Default::default()
        }
    }

    pub(super) fn dest<P: AsRef<Path>>(mut self, dest: P) -> Result<Self> {
        let dest = dest.as_ref();
        self.destdir.push(dest.strip_prefix("/").unwrap_or(dest));
        self.dirs([&self.destdir])?;
        Ok(self)
    }

    fn parse_options<I, T>(&self, options: I) -> (InstallOptions, bool)
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        let options: Vec<String> = options.into_iter().map(|s| s.into()).collect();
        let mut to_parse = vec!["install"];
        to_parse.extend(options.iter().map(|s| s.as_str()));

        let (mut opts, install_cmd) = match InstallOptions::try_parse_from(&to_parse) {
            Ok(opts) => (opts, false),
            Err(_) => (Default::default(), true),
        };
        opts.raw = options;
        (opts, install_cmd)
    }

    pub(super) fn ins_options<I, T>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        let (opts, install_cmd) = self.parse_options(options);
        self.install_cmd = install_cmd;
        self.file_options = Some(opts);
        self
    }

    pub(super) fn dir_options<I, T>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        let (opts, install_cmd) = self.parse_options(options);
        self.install_cmd = install_cmd;
        self.dir_options = Some(opts);
        self
    }

    pub(super) fn prefix<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        let path = path.as_ref();
        let path = path.strip_prefix("/").unwrap_or(path);
        [self.destdir.as_path(), path].iter().collect()
    }

    pub(super) fn link<F, P, Q>(&self, link: F, source: P, target: Q) -> Result<()>
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
        let failed = |e: io::Error| {
            return Err(Error::Base(format!(
                "failed creating link: {source:?} -> {target:?}: {e}"
            )));
        };

        let target = self.prefix(target);

        // overwrite link if it exists
        loop {
            match link(source, &target) {
                Ok(_) => break,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    fs::remove_file(&target).or_else(failed)?
                }
                Err(e) => return failed(e),
            }
        }

        Ok(())
    }

    fn set_attributes<P: AsRef<Path>>(&self, opts: &InstallOptions, path: P) -> Result<()> {
        let path = path.as_ref();
        let uid = opts.uid();
        let gid = opts.gid();
        if uid.is_some() || gid.is_some() {
            unistd::fchownat(None, path, uid, gid, unistd::FchownatFlags::NoFollowSymlink)
                .map_err(|e| Error::Base(format!("failed setting file uid/gid: {path:?}: {e}")))?;
        }

        if let Some(mode) = opts.mode() {
            if !path.is_symlink() {
                stat::fchmodat(None, path, mode, stat::FchmodatFlags::FollowSymlink)
                    .map_err(|e| Error::Base(format!("failed setting file mode: {path:?}: {e}")))?;
            }
        }

        Ok(())
    }

    // Create directories.
    pub(super) fn dirs<I, P>(&self, paths: I) -> Result<()>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        match self.install_cmd {
            false => self.dirs_internal(paths),
            true => self.dirs_cmd(paths),
        }
    }

    // Create directories using internal functionality.
    fn dirs_internal<I, P>(&self, paths: I) -> Result<()>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        for p in paths.into_iter() {
            let path = self.prefix(p);
            fs::create_dir_all(&path)
                .map_err(|e| Error::Base(format!("failed creating dir: {path:?}: {e}")))?;
            if let Some(opts) = &self.dir_options {
                self.set_attributes(opts, path)?;
            }
        }
        Ok(())
    }

    // Create directories using the `install` command.
    fn dirs_cmd<I, P>(&self, paths: I) -> Result<()>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let mut install = Command::new("install");
        install.arg("-d");
        if let Some(opts) = &self.dir_options {
            install.args(&opts.raw);
        }
        install.args(paths.into_iter().map(|p| self.prefix(p)));
        install
            .run()
            .map_or_else(|e| Err(Error::Base(e.to_string())), |_| Ok(()))
    }

    // Install all targets under given directories.
    pub(super) fn from_dirs<I, P>(&self, dirs: I) -> Result<()>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        for dir in dirs.into_iter() {
            let dir = dir.as_ref();
            // Determine whether to skip the base directory -- path.components() can't be used
            // because it normalizes all occurrences of '.' away.
            let depth = match dir.to_string_lossy().ends_with("/.") {
                true => 1,
                false => 0,
            };
            for entry in WalkDir::new(&dir).min_depth(depth) {
                let entry =
                    entry.map_err(|e| Error::Base(format!("error walking {dir:?}: {e}")))?;
                let path = entry.path();
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
                match path.is_dir() {
                    true => self.dirs([dest])?,
                    false => self.files([(path, dest)])?,
                }
            }
        }
        Ok(())
    }

    // Install files.
    pub(super) fn files<I, P, Q>(&self, paths: I) -> Result<()>
    where
        I: IntoIterator<Item = (P, Q)>,
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        match self.install_cmd {
            false => self.files_internal(paths),
            true => self.files_cmd(paths),
        }
    }

    // Install files using internal functionality.
    fn files_internal<I, P, Q>(&self, paths: I) -> Result<()>
    where
        I: IntoIterator<Item = (P, Q)>,
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        for (source, dest) in paths.into_iter() {
            let source = source.as_ref();
            let dest = self.prefix(dest.as_ref());
            let meta = fs::metadata(source)
                .map_err(|e| Error::Base(format!("invalid file {source:?}: {e}")))?;

            // matching `install` command, remove dest before install
            match fs::remove_file(&dest) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => {
                    return Err(Error::Base(format!("failed removing file: {dest:?}: {e}")));
                }
                _ => (),
            }

            fs::copy(source, &dest).map_err(|e| {
                Error::Base(format!("failed copying file: {source:?} to {dest:?}: {e}"))
            })?;
            if let Some(opts) = &self.file_options {
                self.set_attributes(opts, &dest)?;
                if opts.preserve_timestamps {
                    let atime = FileTime::from_last_access_time(&meta);
                    let mtime = FileTime::from_last_modification_time(&meta);
                    set_file_times(&dest, atime, mtime)
                        .map_err(|e| Error::Base(format!("failed setting file time: {e}")))?;
                }
            }
        }
        Ok(())
    }

    // Install files using the `install` command.
    fn files_cmd<I, P, Q>(&self, paths: I) -> Result<()>
    where
        I: IntoIterator<Item = (P, Q)>,
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let mut files = Vec::<(PathBuf, PathBuf)>::new();
        for (source, dest) in paths.into_iter() {
            let source = source.as_ref();
            let dest = dest.as_ref();
            if source.is_symlink() {
                // install symlinks separately since `install` forcibly resolves them
                let source = fs::read_link(source).unwrap();
                self.link(|p, q| symlink(p, q), source, dest)?;
            } else {
                files.push((source.into(), self.prefix(dest)));
            }
        }

        // group and install sets of files by destination to decrease `install` calls
        let files_to_install: Vec<(&Path, &Path)> = files
            .iter()
            .map(|(p, q)| (p.as_path(), q.as_path()))
            .sorted_by_key(|x| x.1)
            .collect();
        for (dest, files_group) in &files_to_install.iter().group_by(|x| x.1) {
            let sources = files_group.map(|x| x.0);
            let mut install = Command::new("install");
            if let Some(opts) = &self.file_options {
                install.args(&opts.raw);
            }
            install.args(sources);
            install.arg(dest);
            install.run().map(|_| ())?;
        }
        Ok(())
    }
}
