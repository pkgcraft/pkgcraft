// explicitly force symbols to be exported
// TODO: https://github.com/rust-lang/rfcs/issues/2771

/// Opaque wrapper for AtomVersion objects.
pub struct AtomVersion;

/// Opaque wrapper for Pkg objects.
pub struct Pkg;
/// Opaque wrapper for Repo objects.
pub struct Repo;

/// Opaque wrapper for PkgIter objects.
pub struct RepoPkgIter;
/// Opaque wrapper for RestrictPkgIter objects.
pub struct RepoRestrictPkgIter;
/// Opaque wrapper for RepoSetPkgIter objects.
pub struct RepoSetPkgIter;

/// Opaque wrapper for Restrict objects.
pub struct Restrict;
