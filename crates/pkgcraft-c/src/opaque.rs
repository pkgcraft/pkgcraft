// explicitly force symbols to be exported
// TODO: https://github.com/rust-lang/rfcs/issues/2771

/// Opaque wrapper for pkgcraft::pkg::Pkg objects.
pub struct Pkg;
/// Opaque wrapper for pkgcraft::repo::Repo objects.
pub struct Repo;

/// Opaque wrapper for pkgcraft::repo::IterCpv objects.
pub struct RepoIterCpv;

/// Opaque wrapper for pkgcraft::repo::Iter objects.
pub struct RepoIter;

/// Opaque wrapper for pkgcraft::repo::IterRestrict objects.
pub struct RepoIterRestrict;

/// Opaque wrapper for pkgcraft::repo::set::Iter objects.
pub struct RepoSetIter;

/// Opaque wrapper for pkgcraft::restrict::Restrict objects.
pub struct Restrict;
