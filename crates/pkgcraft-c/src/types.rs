pub type RepoIter<'a> = pkgcraft::repo::Iter<'a>;
pub type RepoIterCpv<'a> = pkgcraft::repo::IterCpv<'a>;
pub type RepoIterRestrict<'a> = pkgcraft::repo::IterRestrict<'a>;
pub type RepoSetIter<'a> = pkgcraft::repo::set::Iter<'a>;
pub type EbuildTempRepo = pkgcraft::repo::ebuild::temp::Repo;

use pkgcraft::dep::version::Revision as RevisionOwned;
pub type Revision = RevisionOwned<String>;

use pkgcraft::dep::version::Version as VersionOwned;
pub type Version = VersionOwned<String>;

use pkgcraft::dep::cpn::Cpn as CpnOwned;
pub type Cpn = CpnOwned<String>;

use pkgcraft::dep::cpv::Cpv as CpvOwned;
pub type Cpv = CpvOwned<String>;

use pkgcraft::dep::pkg::Dep as DepOwned;
pub type Dep = DepOwned<String>;

/// Generic set operations.
#[repr(C)]
pub enum SetOp {
    And,
    Or,
    Xor,
    Sub,
}
