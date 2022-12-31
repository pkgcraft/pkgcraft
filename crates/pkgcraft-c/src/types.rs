use pkgcraft::{atom, repo};

pub type AtomVersion = atom::Version;
pub type RepoPkgIter<'a> = repo::PkgIter<'a>;
pub type RepoRestrictPkgIter<'a> = repo::RestrictPkgIter<'a>;
pub type RepoSetPkgIter<'a> = repo::set::PkgIter<'a>;
