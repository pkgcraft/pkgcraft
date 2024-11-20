pub type RepoIter = pkgcraft::repo::Iter;
pub type RepoIterCpv = pkgcraft::repo::IterCpv;
pub type RepoIterRestrict = pkgcraft::repo::IterRestrict;
pub type RepoSetIter = pkgcraft::repo::set::Iter;
pub type RepoSetIterRestrict = pkgcraft::repo::set::IterRestrict;

/// Generic set operations.
#[repr(C)]
pub enum SetOp {
    And,
    Or,
    Xor,
    Sub,
}
