use std::ops::Deref;
use std::path::Path;
use std::str::FromStr;

use camino::Utf8DirEntry;
use nix::{sys::stat, unistd};
use walkdir::{DirEntry, WalkDir};

use crate::Error;

#[derive(Debug, Clone)]
pub(crate) struct Group {
    inner: unistd::Group,
}

impl FromStr for Group {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match unistd::Group::from_name(s) {
            Ok(Some(val)) => Ok(Group { inner: val }),
            Ok(None) => Err(Error::InvalidValue(format!("unknown group: {s}"))),
            Err(_) => Err(Error::InvalidValue(format!("invalid group: {s}"))),
        }
    }
}

impl Deref for Group {
    type Target = unistd::Group;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Debug, Clone)]
pub(crate) struct User {
    inner: unistd::User,
}

impl Deref for User {
    type Target = unistd::User;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl FromStr for User {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match unistd::User::from_name(s) {
            Ok(Some(val)) => Ok(User { inner: val }),
            Ok(None) => Err(Error::InvalidValue(format!("unknown user: {s}"))),
            Err(_) => Err(Error::InvalidValue(format!("invalid user: {s}"))),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Mode {
    inner: stat::Mode,
}

impl Deref for Mode {
    type Target = stat::Mode;

    fn deref(&self) -> &Self::Target {
        &self.inner
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
        Ok(Mode { inner: mode })
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

pub(crate) fn is_dir(entry: &DirEntry) -> bool {
    entry.path().is_dir()
}

pub(crate) fn is_file(entry: &DirEntry) -> bool {
    entry.path().is_file()
}

pub(crate) fn has_ext(entry: &DirEntry, ext: &str) -> bool {
    match entry.path().extension() {
        Some(e) => e.to_str() == Some(ext),
        _ => false,
    }
}

pub(crate) fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
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
