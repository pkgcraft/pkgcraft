pub type RepoIter<'a> = pkgcraft::repo::Iter<'a>;
pub type RepoIterCpv<'a> = pkgcraft::repo::IterCpv<'a>;
pub type RepoIterRestrict<'a> = pkgcraft::repo::IterRestrict<'a>;
pub type RepoSetIter<'a> = pkgcraft::repo::set::Iter<'a>;
pub type EbuildTempRepo = pkgcraft::repo::ebuild::temp::Repo;

use pkgcraft::dep::version::Revision as RevisionWrapper;
pub type Revision = RevisionWrapper<String>;

/// Generic set operations.
#[repr(C)]
pub enum SetOp {
    And,
    Or,
    Xor,
    Sub,
}
