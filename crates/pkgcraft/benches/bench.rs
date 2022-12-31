use criterion::*;

mod atom;
mod depset;
mod repo;
mod version;

criterion_group!(atom, atom::bench_pkg_atoms);
criterion_group!(repo, repo::bench_repo_ebuild);
criterion_group!(depset, depset::bench_depsets);
criterion_group!(version, version::bench_pkg_versions);

criterion_main!(atom, repo, depset, version);
