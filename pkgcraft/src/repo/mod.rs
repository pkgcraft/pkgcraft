use std::collections::{HashMap, HashSet};
use std::fmt;

mod ebuild;
mod fake;

type VersionCache = HashMap<String, HashSet<String>>;
type PkgCache = HashMap<String, VersionCache>;

pub trait Repo: fmt::Display {
    // TODO: convert to `impl Iterator` return type once supported within traits
    // https://github.com/rust-lang/rfcs/blob/master/text/1522-conservative-impl-trait.md
    fn categories(&self) -> Box<dyn Iterator<Item = &String> + '_>;
    fn packages<S: AsRef<str>>(&self, cat: S) -> Box<dyn Iterator<Item = &String> + '_>;
    fn versions<S: AsRef<str>>(&self, cat: S, pkg: S) -> Box<dyn Iterator<Item = &String> + '_>;
}
