use pkgcraft::repo;

pub type RepoPkgIter<'a> = repo::PkgIter<'a>;
pub type RepoRestrictPkgIter<'a> = repo::RestrictPkgIter<'a>;
pub type RepoSetPkgIter<'a> = repo::set::PkgIter<'a>;
