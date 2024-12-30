use std::env;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{is_separator, Component, Path, PathBuf};

use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use digest::Digest;

use crate::Error;

/// Limit parallel jobs to the number of logical CPUs on a system. All CPUs are used if jobs is 0.
pub fn bounded_jobs(jobs: usize) -> usize {
    let cpus = num_cpus::get();
    if jobs > 0 && jobs <= cpus {
        jobs
    } else {
        cpus
    }
}

/// Return the hash of a given hashable object.
pub fn hash<T: Hash>(obj: T) -> u64 {
    let mut hasher = DefaultHasher::new();
    obj.hash(&mut hasher);
    hasher.finish()
}

/// Hash the given data using a specified digest function and return the hex-encoded value.
pub(crate) fn digest<D: Digest>(data: &[u8]) -> String {
    let mut hasher = D::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Get the current working directory as a Utf8PathBuf.
pub fn current_dir() -> crate::Result<Utf8PathBuf> {
    let dir = env::current_dir()
        .map_err(|e| Error::InvalidValue(format!("can't get current dir: {e}")))?;
    Utf8PathBuf::try_from(dir)
        .map_err(|e| Error::InvalidValue(format!("invalid unicode path: {e}")))
}

/// Find and return the first existing path from a list of paths, otherwise return None.
pub(crate) fn find_existing_path<I>(paths: I) -> Option<Utf8PathBuf>
where
    I: IntoIterator,
    I::Item: AsRef<Utf8Path>,
{
    for p in paths {
        let path = p.as_ref();
        if let Ok(true) = path.try_exists() {
            return Some(path.into());
        }
    }
    None
}

/// Determines if a path is a single component with no separators.
pub(crate) fn is_single_component<S: AsRef<str>>(path: S) -> bool {
    !path.as_ref().contains(is_separator)
}

/// Construct a relative path from a base directory to the specified path.
//
// Adapted from rustc's old path_relative_from()
// https://github.com/rust-lang/rust/blob/e1d0de82cc40b666b88d4a6d2c9dcbc81d7ed27f/src/librustc_back/rpath.rs#L116-L158
//
// Copyright 2012-2015 The Rust Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
pub fn relpath<P, B>(path: P, base: B) -> Option<PathBuf>
where
    P: AsRef<Path>,
    B: AsRef<Path>,
{
    let path = path.as_ref();
    let base = base.as_ref();

    if path.is_absolute() != base.is_absolute() {
        if path.is_absolute() {
            Some(PathBuf::from(path))
        } else {
            None
        }
    } else {
        let mut ita = path.components();
        let mut itb = base.components();
        let mut comps: Vec<Component> = vec![];
        loop {
            match (ita.next(), itb.next()) {
                (None, None) => break,
                (Some(a), None) => {
                    comps.push(a);
                    comps.extend(ita);
                    break;
                }
                (None, _) => comps.push(Component::ParentDir),
                (Some(a), Some(b)) if comps.is_empty() && a == b => (),
                (Some(a), Some(Component::CurDir)) => comps.push(a),
                (Some(_), Some(Component::ParentDir)) => return None,
                (Some(a), Some(_)) => {
                    comps.push(Component::ParentDir);
                    comps.extend(itb.map(|_| Component::ParentDir));
                    comps.push(a);
                    comps.extend(ita);
                    break;
                }
            }
        }
        Some(comps.iter().collect())
    }
}

/// Construct a relative utf8 path from a base directory to the specified path.
pub fn relpath_utf8<P, B>(path: P, base: B) -> Option<Utf8PathBuf>
where
    P: AsRef<Utf8Path>,
    B: AsRef<Utf8Path>,
{
    let path = path.as_ref();
    let base = base.as_ref();

    if path.is_absolute() != base.is_absolute() {
        if path.is_absolute() {
            Some(Utf8PathBuf::from(path))
        } else {
            None
        }
    } else {
        let mut ita = path.components();
        let mut itb = base.components();
        let mut comps: Vec<Utf8Component> = vec![];
        loop {
            match (ita.next(), itb.next()) {
                (None, None) => break,
                (Some(a), None) => {
                    comps.push(a);
                    comps.extend(ita);
                    break;
                }
                (None, _) => comps.push(Utf8Component::ParentDir),
                (Some(a), Some(b)) if comps.is_empty() && a == b => (),
                (Some(a), Some(Utf8Component::CurDir)) => comps.push(a),
                (Some(_), Some(Utf8Component::ParentDir)) => return None,
                (Some(a), Some(_)) => {
                    comps.push(Utf8Component::ParentDir);
                    comps.extend(itb.map(|_| Utf8Component::ParentDir));
                    comps.push(a);
                    comps.extend(ita);
                    break;
                }
            }
        }
        Some(comps.iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relpaths() {
        for (path, base, expected) in [
            ("path", "path", Some("")),
            ("/path", "path", Some("/path")),
            ("path", "/path", None),
            ("/path", "/path", Some("")),
            ("", "", Some("")),
            ("/", "", Some("/")),
            ("", "/", None),
            ("/", "path", Some("/")),
            ("path/file", "./path", Some("path/../file")),
            ("path/file", "path/../file", None),
            ("path", "/", None),
            ("/path/to/file", "/path/to", Some("file")),
            ("/path/to/file", "/path/to/", Some("file")),
        ] {
            // utf8
            assert_eq!(
                relpath_utf8(path, base).map(|x| x.to_string()).as_deref(),
                expected,
                "relpath failed: path {path:?}, base {base:?}"
            );

            // non-utf8
            assert_eq!(
                relpath(path, base)
                    .map(|x| x.to_str().unwrap().to_string())
                    .as_deref(),
                expected,
                "relpath failed: path {path:?}, base {base:?}"
            );
        }
    }
}
