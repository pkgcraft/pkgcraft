use criterion::*;

mod depset;
mod pkgdep;
mod repo;
mod version;

criterion_group!(pkgdep, pkgdep::bench_pkg_deps);
criterion_group!(repo, repo::bench_repo_ebuild);
criterion_group!(depset, depset::bench_depsets);
criterion_group!(version, version::bench_pkg_versions);

criterion_main!(pkgdep, repo, depset, version);
