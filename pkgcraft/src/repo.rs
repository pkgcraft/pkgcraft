use std::collections::{HashMap, HashSet};
use std::fmt;
use std::iter;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

mod ebuild;
mod fake;

type VersionMap = HashMap<String, HashSet<String>>;
type PkgMap = HashMap<String, VersionMap>;
type StringIter<'a> = Box<dyn Iterator<Item = &'a String> + 'a>;

#[derive(Debug, Default, PartialEq, Deserialize, Serialize)]
struct PkgCache {
    pkgmap: PkgMap,
}

impl PkgCache {
    fn categories(&self) -> StringIter {
        Box::new(self.pkgmap.keys())
    }

    fn packages<S: AsRef<str>>(&self, cat: S) -> StringIter {
        match self.pkgmap.get(cat.as_ref()) {
            Some(pkgs) => Box::new(pkgs.keys()),
            None => Box::new(iter::empty::<&String>()),
        }
    }

    fn versions<S: AsRef<str>>(&self, cat: S, pkg: S) -> StringIter {
        match self.pkgmap.get(cat.as_ref()) {
            Some(pkgs) => match pkgs.get(pkg.as_ref()) {
                Some(vers) => Box::new(vers.iter()),
                None => Box::new(iter::empty::<&String>()),
            },
            None => Box::new(iter::empty::<&String>()),
        }
    }
}

pub fn from_path<S: AsRef<str>>(id: S, path: S) -> Result<(String, Box<dyn Repo>)> {
    let id = id.as_ref();
    let path = path.as_ref();

    if let Ok(repo) = ebuild::Repo::from_path(id, path) {
        return Ok(("ebuild".to_string(), Box::new(repo)));
    }

    if let Ok(repo) = fake::Repo::from_path(id, path) {
        return Ok(("fake".to_string(), Box::new(repo)));
    }

    Err(Error::ConfigError(format!(
        "{:?} repo at {:?}: unknown or invalid format",
        id, path
    )))
}

pub fn from_format<S: AsRef<str>>(id: S, path: S, format: S) -> Result<Box<dyn Repo>> {
    let id = id.as_ref();
    let path = path.as_ref();
    let format = format.as_ref();

    match format {
        "ebuild" => Ok(Box::new(ebuild::Repo::from_path(id, path)?)),
        "fake" => Ok(Box::new(fake::Repo::from_path(id, path)?)),
        _ => {
            let err = format!("{:?} repo: unknown format: {:?}", id, format);
            Err(Error::ConfigError(err))
        }
    }
}

static SUPPORTED_FORMATS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["ebuild", "fake"].iter().cloned().collect());

pub fn is_supported<S: AsRef<str>>(s: S) -> Result<()> {
    let s = s.as_ref();
    match SUPPORTED_FORMATS.get(s) {
        Some(_) => Ok(()),
        None => Err(Error::ConfigError(format!("unknown repo format: {:?}", s))),
    }
}

pub trait Repo: fmt::Debug + fmt::Display {
    // TODO: convert to `impl Iterator` return type once supported within traits
    // https://github.com/rust-lang/rfcs/blob/master/text/1522-conservative-impl-trait.md
    fn categories(&mut self) -> StringIter;
    fn packages(&mut self, cat: &str) -> StringIter;
    fn versions(&mut self, cat: &str, pkg: &str) -> StringIter;
}

impl<R: Repo + ?Sized> Repo for Box<R> {
    #[inline]
    fn categories(&mut self) -> StringIter {
        (**self).categories()
    }

    #[inline]
    fn packages(&mut self, cat: &str) -> StringIter {
        (**self).packages(cat)
    }

    #[inline]
    fn versions(&mut self, cat: &str, pkg: &str) -> StringIter {
        (**self).versions(cat, pkg)
    }
}
