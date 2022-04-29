use std::path::Path;

use walkdir::{DirEntry, WalkDir};

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
