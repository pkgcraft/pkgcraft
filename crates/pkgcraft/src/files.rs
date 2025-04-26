use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use camino::{Utf8DirEntry, Utf8Path};
use itertools::Itertools;
use nix::{sys::stat, unistd};
use walkdir::{DirEntry, WalkDir};

use crate::Error;
use crate::utils::relpath;

#[derive(Debug, Clone)]
pub(crate) struct Group(unistd::Group);

impl FromStr for Group {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match unistd::Group::from_name(s) {
            Ok(Some(val)) => Ok(Group(val)),
            Ok(None) => Err(Error::InvalidValue(format!("unknown group: {s}"))),
            Err(_) => Err(Error::InvalidValue(format!("invalid group: {s}"))),
        }
    }
}

impl Deref for Group {
    type Target = unistd::Group;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub(crate) struct User(unistd::User);

impl Deref for User {
    type Target = unistd::User;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for User {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match unistd::User::from_name(s) {
            Ok(Some(val)) => Ok(User(val)),
            Ok(None) => Err(Error::InvalidValue(format!("unknown user: {s}"))),
            Err(_) => Err(Error::InvalidValue(format!("invalid user: {s}"))),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Mode(stat::Mode);

impl Deref for Mode {
    type Target = stat::Mode;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for Mode {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let without_prefix = s.trim_start_matches("0o");
        let mode = stat::mode_t::from_str_radix(without_prefix, 8)
            .map_err(|_| Error::InvalidValue(format!("invalid mode: {s}")))?;
        let mode = stat::Mode::from_bits(mode)
            .ok_or_else(|| Error::InvalidValue(format!("invalid mode: {s}")))?;
        Ok(Mode(mode))
    }
}

// None value coerced to a directory filtering predicate function pointer for use with
// Option-wrapped closure parameter generics.
type WalkDirFilter = fn(&DirEntry) -> bool;
pub(crate) const NO_WALKDIR_FILTER: Option<WalkDirFilter> = None;

pub(crate) fn sorted_dir_list<P: AsRef<Path>>(path: P) -> WalkDir {
    WalkDir::new(path.as_ref())
        .sort_by_file_name()
        .min_depth(1)
        .max_depth(1)
}

/// Return an iterator of all the relative paths to files under a path.
pub(crate) fn relative_paths<'a, P>(path: P) -> impl Iterator<Item = PathBuf> + 'a
where
    P: AsRef<Path> + Copy + 'a,
{
    WalkDir::new(path)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .filter_map(move |e| relpath(e.path(), path))
}

pub(crate) fn is_file(entry: &DirEntry) -> bool {
    entry.path().is_file()
}

pub(crate) fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

pub(crate) fn sorted_dir_list_utf8(path: &Utf8Path) -> crate::Result<Vec<Utf8DirEntry>> {
    let mut entries: Vec<_> = path
        .read_dir_utf8()
        .map_err(|e| Error::IO(format!("failed reading dir: {path}: {e}")))?
        .try_collect()?;
    entries.sort_by(|a, b| a.file_name().cmp(b.file_name()));
    Ok(entries)
}

pub(crate) fn is_dir_utf8(entry: &Utf8DirEntry) -> bool {
    entry.path().is_dir()
}

pub(crate) fn is_file_utf8(entry: &Utf8DirEntry) -> bool {
    entry.path().is_file()
}

pub(crate) fn is_hidden_utf8(entry: &Utf8DirEntry) -> bool {
    entry.file_name().starts_with('.')
}

pub(crate) fn has_ext_utf8(entry: &Utf8DirEntry, ext: &str) -> bool {
    entry
        .path()
        .extension()
        .map(|s| s == ext)
        .unwrap_or_default()
}

/// Determine if a [`Utf8DirEntry`] is a valid ebuild file.
pub(crate) fn is_ebuild(entry: &Utf8DirEntry) -> bool {
    is_file_utf8(entry) && !is_hidden_utf8(entry) && has_ext_utf8(entry, "ebuild")
}

/// Create a file atomically by writing to a temporary path and then renaming it.
pub(crate) fn atomic_write_file<C: AsRef<[u8]>, P: AsRef<Utf8Path>>(
    path: P,
    data: C,
) -> crate::Result<()> {
    let path = path.as_ref();

    // create parent dir
    let dir = path
        .parent()
        .ok_or_else(|| Error::IO(format!("invalid file path: {path}")))?;
    fs::create_dir_all(dir)
        .map_err(|e| Error::IO(format!("failed creating dir: {dir}: {e}")))?;

    // TODO: support custom temporary file path formats
    let pid = std::process::id();
    let file_name = path
        .file_name()
        .ok_or_else(|| Error::IO(format!("invalid file path: {path}")))?;
    let temp = dir.join(format!(".{file_name}.{pid}"));

    // write to the temporary file
    fs::write(&temp, data)
        .map_err(|e| Error::IO(format!("failed writing data: {temp}: {e}")))?;

    // move file to final path
    fs::rename(&temp, path)
        .map_err(|e| Error::IO(format!("failed renaming file: {temp} -> {path}: {e}")))?;

    Ok(())
}
