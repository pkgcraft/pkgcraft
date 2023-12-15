// explicitly force symbols to be exported
// TODO: https://github.com/rust-lang/rfcs/issues/2771

/// Opaque wrapper for pkgcraft::pkg::Pkg objects.
pub struct Pkg;
/// Opaque wrapper for pkgcraft::repo::Repo objects.
pub struct Repo;
/// Opaque wrapper for pkgcraft::repo::temp::Repo objects.
pub struct EbuildTempRepo;

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

/// Opaque wrapper for pkgcraft::dep::version::Revision<String> objects.
pub struct Revision;

/// Opaque wrapper for pkgcraft::dep::version::Version<String> objects.
pub struct Version;

/// Opaque wrapper for pkgcraft::dep::cpv::Cpv<String> objects.
pub struct Cpv;

/// Opaque wrapper for pkgcraft::dep::pkg::Dep<String> objects.
pub struct Dep;
